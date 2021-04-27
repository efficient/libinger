#include "config.h"
#include "globals.h"

#include "goot.h"
#include "handle.h"
#include "namespace.h"
#include "plot.h"

#include <elfutils/libasm.h>
#include <assert.h>
#include <ctype.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <threads.h>

void procedure_linkage_override(void);

struct disasm {
	size_t register_index;
	bool saw_call_instr;
};

static DisasmCtx_t *ctx;
static Ebl ebl;
static struct sigaction handler;

static void __attribute__((noinline)) libgotcha_traceglobal(uintptr_t before, uintptr_t after) {
	FILE *err = config_traceglobals();
	if(err)
		fprintf(err, "libgotcha trace: rerouted memory access from %#lx to %#lx\n",
			before, after);
}

static void unresolvable_global(const char *message) {
	assert(message);
	fputs(message, stderr);
	abort();
}

// If an instruction contains a memory access to an location computed from one or more
// general-purpose registers, update reg to store the (64-bit) ucontext/mcontext index of the
// register used as the *base* in the address calculation; otherwise, set it to -1.  Note that only
// 64-bit registers are eligible for selection, and that the stack pointer is *not* considered to be
// a general-purpose register for this purpose, in contrast to the base pointer!
static int addr_calc_base_gpr(char *str, size_t len, void *reg) {
	(void) len;

	struct disasm *res = reg;
	size_t *greg = &res->register_index;
	const char *base = strstr(str, "(%r");
	*greg = -1;
	if(base) {
		switch(*(base += 3)) {
		case '1':
			if(base[1] < '0' || base[1] > '5') {
				fputs_unlocked("libgotcha warning: unrecognized %r1N register\n",
					stderr);
				break;
			}

			if(!isalpha(base[2])) {
				if(!ispunct(base[2])) {
					fputs_unlocked("libgotcha warning: unknown %rNN suffix\n",
						stderr);
					break;
				}
				*greg = base[1] - '0' + REG_R10;
			}
			break;
		case '8':
		case '9':
			if(!isalpha(base[1])) {
				if(!ispunct(base[1])) {
					fputs_unlocked("libgotcha warning: unknown %rN suffix\n",
						stderr);
					break;
				}
				*greg = base[0] - '8' + REG_R8;
			}
			break;
		case 'a':
			if(base[1] != 'x') {
				fputs_unlocked("libgotcha warning: unrecognized %raX register\n",
					stderr);
				break;
			}
			*greg = REG_RAX;
			break;
		case 'b':
			switch(base[1]) {
			case 'p':
				*greg = REG_RBP;
				break;
			case 'x':
				*greg = REG_RBX;
				break;
			default:
				fputs_unlocked("libgotcha warning: unrecognized %rbX register\n",
					stderr);
				break;
			}
			break;
		case 'c':
			if(base[1] != 'x') {
				fputs_unlocked("libgotcha warning: unrecognized %rcX register\n",
					stderr);
				break;
			}
			*greg = REG_RCX;
			break;
		case 'd':
			switch(base[1]) {
			case 'i':
				*greg = REG_RDI;
				break;
			case 'x':
				*greg = REG_RDX;
				break;
			default:
				fputs_unlocked("libgotcha warning: unrecognized %rdX register\n",
					stderr);
				break;
			}
			break;
		case 's':
			switch(base[1]) {
			case 'i':
				*greg = REG_RSI;
				break;
			case 'p':
				// Ignore the stack pointer.
				break;
			default:
				fputs_unlocked("libgotcha warning: unrecognized %rsX register\n",
					stderr);
				break;
			}
		}
	}
	res->saw_call_instr = !strncmp(str, "call", 4);

	// Stop after processing the first instruction.
	return 1;
}

// Disassembles the instruction at address inst, updating:
//  * inst to point at the following instruction (which in some cases will be a return address)
//  * base_addr_reg with the index of the general-purpose register containing the base of the
//    address calculation, or -1 if no displacement-mode memory address operand was found
//  * its return value to indicate whether it happened to process a call instruction
static inline bool disasm_instr(const uint8_t **inst, size_t *base_addr_reg) {
	struct disasm res;
	disasm_cb(ctx, inst, *inst + X86_64_MAX_INSTR_LEN, 0, "%m %.1o,%.2o,%.3o",
		addr_calc_base_gpr, &res, NULL);
	*base_addr_reg = res.register_index;
	return res.saw_call_instr;
}

static void segv(int no, siginfo_t *si, void *co) {
	static thread_local size_t last_reg;
	static thread_local gregset_t last_regs;
	static thread_local uintptr_t last_next;
	static thread_local uintptr_t last_old;
	static thread_local uintptr_t last_new;

	// Switch to the base namespace while we're in this function.  Note that
	// procedure_linkage_override() isn't currently aware of register spill during argument
	// passing, so without this the call to disasm_cb() below can fail.
	Lmid_t *nsp = namespace_thread();
	Lmid_t ns = *nsp;
	*nsp = LM_ID_BASE;

	bool next = true;
	const char *error = NULL;
	int erryes = errno;
	ucontext_t *uc = co;
	mcontext_t *mc = &uc->uc_mcontext;
	size_t greg;
	const uint8_t *inst = (uint8_t *) mc->gregs[REG_RIP];
	if(inst == si->si_addr) {
		// Woah!  The error was in executing this instruction, which probably means it's
		// unreadable.  Let's *not* try to disassemble it; instead, we'll see whether we
		// just followed a misguided indirect call instruction.  First search backward from
		// the return address for such an instruction...
		bool hit = false;
		const uint8_t *cur;
		const uint8_t *retaddr = *(uint8_t **) mc->gregs[REG_RSP];
		for(cur = inst = retaddr - 1;
			cur >= retaddr - X86_64_MAX_INSTR_LEN &&
			(!disasm_instr(&inst, &greg) || inst != retaddr || (signed) greg == -1 ||
			!(hit = true));
			inst = --cur);

		if(hit) {
			// We found a plausible instruction.  At this point instr has been set back
			// to the return address, but cur points to the call instruction we need to
			// reexecute, and greg contains the index of the base-address register from
			// its displacement-mode memory address operand.
			mc->gregs[REG_RIP] = (uintptr_t) cur;
			mc->gregs[REG_RSP] += 8;
		} else
			goto out;
	} else {
		disasm_instr(&inst, &greg);
		if((signed) greg == -1)
			goto out;
	}

	uintptr_t addr = mc->gregs[greg];
	if(!addr) {
		error = "libgotcha error: pointer dereference with NULL base address\n";
		goto out;
	}

	uintptr_t pagesize = plot_pagesize();
	size_t index = addr & (pagesize - 1);
	const struct plot *plot = (struct plot *) (addr - index - pagesize);
	if(index >= PLOT_ENTRIES_PER_PAGE || plot->resolver != procedure_linkage_override) {
		// It looks like the base of the displacement-mode address calculation isn't one we
		// instrumented.  Maybe the code computed a custom base address using a register
		// that we've since updated due to a separate (but nearby) dereference?
		if(!last_old) {
			// I guess not: we haven't actually resolved any addresses before.  Bail!
			error = "libgotcha error: attempted to dereference unrecognized global\n";
			goto out;
		}

		// See whether we can apply one of our heuristics.  Note that they currently assume
		// that the code applied a *linear* offset to the last address we resolved.
		uintptr_t *retaddr = (uintptr_t *) mc->gregs[REG_RSP];
		if(greg == last_reg || (uintptr_t) mc->gregs[last_reg] == last_new ||
			(*retaddr == last_next && mc->gregs[greg] == last_regs[greg])) {
			// Let's try adding the same offset we did last time we resolved an address,
			// because we're in one of the following common situations:
			//  * The client code is using the same base address register as it was
			//    during our last trip through this function.  This might indicate that
			//    said code is using the register as an address accumulator, but doing
			//    so in concert with some other temporary register: because of this
			//    indirection, overwriting the register with the temporary after we
			//    had preformed the original address resolution would have left us
			//    unable to process any subsequent values accumulated into the register.
			//  * The base address register is different than the one updated during our
			//    last trip through this function, but the value of the latter has
			//    remained unchanged since we updated it.  Because it contains a memory
			//    address we had to resolve, this strongly suggests that the client code
			//    has only executed a few instructions since then, which we can infer
			//    even if that set included one or more branch instructions.
			//  * The current return address points to the instruction immediately
			//    following the one that faulted to result in the last invocation of
			//    this function, and the current base address register's value has
			//    remained the same since the faulting instruction was executed.  This
			//    implies that said instruction was an indirect procedure call, and that
			//    the register was probably just used to pass a pointer argument.
			//    Because we didn't resolve the address of the indirect call until the
			//    client code was already transferring control, there was no way for it
			//    to have passed a pointer without performing arithmetic directly on the
			//    dummy address present before the call.
			ptrdiff_t offset = addr - last_old;
			mc->gregs[greg] = last_new + offset;
			next = false;
		} else
			error = "libgotcha error: unrecognized global and no heuristic pertains\n";
		goto out;
	}

	const struct goot *goot = plot->goot;
	const union goot_entry *entry = goot->entries + index;
	if(entry->free.odd_tag & 0x1) {
		error = "libgotcha error: access to global address backed by dangling GOOT entry\n";
		goto out;
	}

	const struct handle *handle = entry->lib;
	const struct shadow_gots *shadow = &handle->shadow;
	index += goot->adjustment;
	if(goot->identifier == shadow->override_table) {
		index += shadow->last_adjustment;
		index -= shadow->first_entry;
	}

	uintptr_t dest = shadow->gots[ns][index];
	if(!dest)
		dest = shadow->gots[LM_ID_BASE][index];
	if(!dest) {
		error = "libgotcha error: access to global backed by NULL pointer\n";
		goto out;
	}

	libgotcha_traceglobal(addr, dest);
	mc->gregs[greg] = dest;
	if(next) {
		// We resolved the address without applying a heuristic.  Save a record of what we
		// changed and how to allow us to heuristically resolve addresses based on our
		// experience.  Because this is guarded, heuristics cannot chain, but multiple of
		// them can be triggered based on a common base resolution.
		last_reg = greg;
		memcpy(last_regs, &mc->gregs, sizeof last_regs);
		last_next = (uintptr_t) inst;
		last_old = addr;
		last_new = dest;
	}
	next = false;

out:
	*nsp = ns;
	if(next) {
		if(error && config_abortsegv()) {
			uintptr_t *sp = (uintptr_t *) (mc->gregs[REG_RSP] -= 8);
			mc->gregs[REG_RIP] = (uintptr_t) unresolvable_global;
			mc->gregs[REG_RDI] = (uintptr_t) error;
			*sp = (uintptr_t) inst;
		} else
			handler.sa_sigaction(no, si, co);
	}
	errno = erryes;
}

enum error globals_init(void) {
	// Because we call this while rerouting each global variable access, and because it uses
	// (and caches) stderr, we must bootstrap it before we begin intercepting such accesses.
	config_traceglobals();

	// Likewise, because of its use of getenv().
	config_abortsegv();

	sigset_t mask;
	sigfillset(&mask);
	sigdelset(&mask, SIGSEGV);

	struct sigaction old;
	struct sigaction new = {
		.sa_flags = SA_SIGINFO,
		.sa_mask = mask,
		.sa_sigaction = segv,
	};
	if(sigaction(SIGSEGV, &new, &old))
		return ERROR_SIGACTION;

	assert((uintptr_t) old.sa_handler == (uintptr_t) old.sa_sigaction);
	if(old.sa_handler)
		memcpy(&handler, &old, sizeof handler);

	if(!ctx) {
		x86_64_init(NULL, 0, &ebl, sizeof ebl);
		if(!(ctx = disasm_begin(&ebl, NULL, NULL)))
			return ERROR_LIBASM;
	}

	return SUCCESS;
}

enum error globals_deinit(void) {
	if(handler.sa_handler) {
		if(sigaction(SIGSEGV, &handler, NULL))
			return ERROR_SIGACTION;
		memset(&handler, 0, sizeof handler);
	}

	if(ctx)
		disasm_end(ctx);

	return SUCCESS;
}

struct sigaction *globals_handler(void) {
	return &handler;
}
