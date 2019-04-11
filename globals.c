#include "globals.h"

#include "goot.h"
#include "handle.h"
#include "namespace.h"
#include "plot.h"
#include "threads.h"

#include <elfutils/libasm.h>
#include <assert.h>
#include <ctype.h>
#include <stdio.h>
#include <string.h>

void procedure_linkage_override(void);

static DisasmCtx_t *ctx;
static Ebl ebl;
static void (*handler)(int, siginfo_t *, void *);

// If an instruction contains a memory access to an location computed from one or more
// general-purpose registers, update reg to store the (64-bit) ucontext/mcontext index of the
// register used as the *base* in the address calculation; otherwise, set it to -1.  Note that only
// 64-bit registers are eligible for selection, and that the stack pointer is *not* considered to be
// a general-purpose register for this purpose, in contrast to the base pointer!
static int addr_calc_base_gpr(char *str, size_t len, void *reg) {
	(void) len;

	size_t *greg = reg;
	const char *base = strstr(str, "(%r");
	*greg = -1;
	if(base) {
		switch(*(base += 3)) {
		case '1':
			if(base[1] < '0' || base[1] > '5') {
				fputs_unlocked("libgotcha warning: unrecognized %r1N register",
					stderr);
				break;
			}

			if(!isalpha(base[2])) {
				if(!ispunct(base[2])) {
					fputs_unlocked("libgotcha warning: unknown %rNN suffix",
						stderr);
					break;
				}
				*greg = base[1] - '0' + REG_R10;
			}
			break;
		case '8':
		case '9':
			if(!isalpha(base[1])) {
				if(!ispunct(base[2])) {
					fputs_unlocked("libgotcha warning: unknown %rN suffix",
						stderr);
					break;
				}
				*greg = base[0] - '8' + REG_R8;
			}
			break;
		case 'a':
			if(base[1] != 'x') {
				fputs_unlocked("libgotcha warning: unrecognized %raX register",
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
				fputs_unlocked("libgotcha warning: unrecognized %rbX register",
					stderr);
				break;
			}
			break;
		case 'c':
			if(base[1] != 'x') {
				fputs_unlocked("libgotcha warning: unrecognized %rcX register",
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
				fputs_unlocked("libgotcha warning: unrecognized %rdX register",
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
				fputs_unlocked("libgotcha warning: unrecognized %rsX register",
					stderr);
				break;
			}
		}
	}

	// Stop after processing the first instruction.
	return 1;
}

static void segv(int no, siginfo_t *si, void *co) {
	static thread_local size_t last_reg;
	static thread_local uintptr_t last_old;
	static thread_local uintptr_t last_new;

	ucontext_t *uc = co;
	mcontext_t *mc = &uc->uc_mcontext;
	size_t greg;
	const uint8_t *inst = (uint8_t *) mc->gregs[REG_RIP];
	disasm_cb(ctx, &inst, inst + X86_64_MAX_INSTR_LEN, 0, "%.1o,%.2o,%.3o",
		addr_calc_base_gpr, &greg, NULL);
	if((signed) greg == -1) {
		handler(no, si, co);
		return;
	}

	uintptr_t pagesize = plot_pagesize();
	uintptr_t addr = mc->gregs[greg];
	size_t index = addr & (pagesize - 1);
	const struct plot *plot = (struct plot *) (addr - index - pagesize);
	if(index >= PLOT_ENTRIES_PER_PAGE || plot->resolver != procedure_linkage_override) {
		if(last_old && (greg == last_reg || (uintptr_t) mc->gregs[last_reg] == last_new)) {
			ptrdiff_t offset = addr - last_old;
			mc->gregs[greg] = last_new + offset;
		} else
			handler(no, si, co);
		return;
	}

	const struct goot *goot = plot->goot;
	const union goot_entry *entry = goot->entries + index;
	if(entry->free.odd_tag & 0x1) {
		fputs_unlocked(
			"libgotcha error: access to global address backed by dangling GOOT entry\n",
			stderr
		);
		handler(no, si, co);
		return;
	}

	const struct handle *handle = entry->lib;
	const struct shadow_gots *shadow = &handle->shadow;
	index += goot->adjustment;
	if(goot->identifier == shadow->override_table) {
		index += shadow->last_adjustment;
		index -= shadow->first_entry;
	}

	Lmid_t namespace = *namespace_thread();
	uintptr_t dest = shadow->gots[namespace][index];
	if(!dest) {
		dest = shadow->gots[LM_ID_BASE][index];
	}
	if(!dest) {
		fputs_unlocked("libgotcha error: access to global backed by NULL pointer",
			stderr);
		handler(no, si, co);
		return;
	}

	mc->gregs[greg] = dest;
	last_reg = greg;
	last_old = addr;
	last_new = dest;
}

enum error globals_init(void) {
	struct sigaction old;
	struct sigaction new = {
		.sa_flags = SA_SIGINFO,
		.sa_sigaction = segv,
	};
	if(sigaction(SIGSEGV, &new, &old))
		return ERROR_SIGACTION;

	assert((uintptr_t) old.sa_handler == (uintptr_t) old.sa_sigaction);
	handler = old.sa_sigaction;

	if(!ctx) {
		x86_64_init(NULL, 0, &ebl, sizeof ebl);
		if(!(ctx = disasm_begin(&ebl, NULL, NULL)))
			return ERROR_LIBASM;
	}

	return SUCCESS;
}

enum error globals_deinit(void) {
	if(handler) {
		struct sigaction restore = {
			.sa_flags = SA_SIGINFO,
			.sa_sigaction = handler,
		};
		if(sigaction(SIGSEGV, &restore, NULL))
			return ERROR_SIGACTION;
		handler = NULL;
	}

	if(ctx)
		disasm_end(ctx);

	return SUCCESS;
}

void globals_install_handler(void (*repl)(int, siginfo_t *, void *)) {
	handler = repl;
}
