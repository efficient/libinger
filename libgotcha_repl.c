#include "config.h"
#include "dynamic.h"
#include "globals.h"
#include "handles.h"
#include "namespace.h"
#include "tcb.h"

#include <asm/prctl.h>
#include <assert.h>
#include <link.h>
#include <signal.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <threads.h>

#pragma GCC visibility push(hidden)
#include "libgotcha_repl.h"
#pragma GCC visibility pop

#pragma weak libgotcha_dlmopen = dlmopen
void *dlmopen(Lmid_t lmid, const char *filename, int flags) {
	if(lmid == LM_ID_BASE)
		return dlmopen(lmid, filename, flags);

	// Trick dlerror() into reporting "invalid target namespace for dlmopen()."
	void *null = dlmopen(NUM_SHADOW_NAMESPACES + 1, "", RTLD_LAZY);
	assert(!null);
	return null;
}

struct ure {
	const char *const self;
	Lmid_t namespace;
	int (*const call)(int (*)(struct dl_phdr_info *, size_t, void *), void *);
	int (*const callback)(struct dl_phdr_info *, size_t, void *);
	void *const data;
};

static int ns_iterate_phdr(struct ure *structure) {
	struct link_map *hand = namespace_get(structure->namespace, structure->self, RTLD_LAZY);
	if(!hand)
		return 0;

	int (*dl_iterate_phdr)(int (*)(struct dl_phdr_info *, size_t, void *), void *) =
		(int (*)(int (*)(struct dl_phdr_info *, size_t, void *), void *)) (uintptr_t)
		dlsym(hand, "dl_iterate_phdr");
	assert(dl_iterate_phdr);
	return dl_iterate_phdr(NULL, structure);
}

#pragma weak libgotcha_dl_iterate_phdr = dl_iterate_phdr
int dl_iterate_phdr(int (*callback)(struct dl_phdr_info *, size_t, void *), void *data) {
	int (*call)(int (*)(struct dl_phdr_info *, size_t, void *), void *) = dl_iterate_phdr;
	struct ure *structure = NULL;
	if(!callback) {
		structure = data;
		call = structure->call;
		callback = structure->callback;
		data = structure->data;
	}

	// We use an indirect call here because, unlike the rest of our functions, this one may
	// execute in an ancillary namespace, where our interposition adjustments do not apply.
	int eger = call(callback, data);
	if(eger)
		return eger;

	// The _Unwind_RaiseException() implementations in libgcc and libunwind call
	// dl_iterate_phdr() to find the .eh_frame section containing the canonical frame
	// information (CFI) metadata.  Unfortunately, ld.so's implementation thereof only searches
	// the invoking namespace, which causes unwinding to fail as soon as it crosses a namespace
	// boundary.  We alter the semantics by extending the search to ancillary namespaces as long
	// as we haven't yet found the callback's desired program header and we haven't encountered
	// a namespace that doesn't appear to contain a copy of ourselves (since we must call
	// ld.so's implementation from that copy in order for it to behave any differently).
	if(!structure) {
		struct ure structure = {
			.self = namespace_self()->l_name,
			.namespace = 1,
			.call = call,
			.callback = callback,
			.data = data,
		};
		return ns_iterate_phdr(&structure);
	} else {
		++structure->namespace;
		return ns_iterate_phdr(structure);
	}
}

void *libgotcha_dl_open(
	const char *filename, int flags, uintptr_t caller, Lmid_t lmid,
	int argc, char **argv, char **env
) {
	return dynamic_open(filename, flags, caller, lmid, argc, argv, env);
}

void libgotcha_dl_close(void *handle) {
	dynamic_close(handle);
}

#pragma weak libgotcha_arch_prctl = arch_prctl
int arch_prctl(int code, uintptr_t addr) {
	int stat = tcb_prctl(code, addr);
	if(stat)
		return stat;

	if(code == ARCH_SET_FS) {
		Lmid_t group = *namespace_caller();
		if(group)
			handles_restoretls(group);
		// else we are in group 0 and will do this on the next libgotcha_group_thread_set()
	}

	return stat;
}

#pragma weak libgotcha_pthread_kill = pthread_kill
int pthread_kill(pthread_t thread, int sig) {
	uintptr_t repl = *tcb_parent();
	if(!repl)
		repl = thread;
	return pthread_kill(repl, sig);
}

static thread_local bool segv_masked;
static void (*segv_handler)(int, siginfo_t *, void *);

static void segv_trampoline(int no, siginfo_t *si, void *co) {
	if(segv_masked) {
		fputs("libgotcha error: Received segmentation fault while the signal was masked\n",
			stderr);
		abort();
	} else
		segv_handler(no, si, co);
}

static inline struct sigaction *setup_segv_trampoline(void) {
	struct sigaction *cfg = globals_handler();
	if(cfg->sa_sigaction != segv_trampoline) {
		segv_handler = cfg->sa_sigaction;
		cfg->sa_sigaction = segv_trampoline;
	}
	return cfg;
}

#pragma weak libgotcha_signal = signal
void (*signal(int signum, void (*handler)(int)))(int) {
	if(signum != SIGSEGV || config_noglobals())
		return signal(signum, handler);

	if(handler == SIG_IGN || handler == SIG_DFL) {
		segv_masked = handler == SIG_IGN;
		return (void (*)(int)) (uintptr_t) segv_handler;
	} else {
		setup_segv_trampoline();

		void (*old)(int) = (void (*)(int)) (uintptr_t) segv_handler;
		segv_handler = (void (*)(int, siginfo_t *, void *)) (uintptr_t) handler;
		return old;
	}
}

#pragma weak libgotcha_sigaction = sigaction
int sigaction(int signum, const struct sigaction *act, struct sigaction *oldact) {
	bool noglobals = config_noglobals();
	if(signum != SIGSEGV || noglobals) {
		struct sigaction sa;
		if(!noglobals && act && sigismember(&act->sa_mask, SIGSEGV)) {
			if(segv_handler)
				fputs("libgotcha warning: "
					"sigaction() ignoring request to block SIGSEGV in handler\n",
					stderr);
			memcpy(&sa, act, sizeof sa);
			sigdelset(&sa.sa_mask, SIGSEGV);
			act = &sa;
		}
		return sigaction(signum, act, oldact);
	}

	struct sigaction *myact = setup_segv_trampoline();
	if(oldact) {
		memcpy(oldact, myact, sizeof *oldact);
		oldact->sa_sigaction = segv_handler;
	}
	if(act) {
		struct sigaction tmpact;
		memcpy(&tmpact, act, sizeof tmpact);
		segv_handler = tmpact.sa_sigaction;
		tmpact.sa_sigaction = segv_trampoline;
		memcpy(myact, &tmpact, sizeof *myact);
	}
	return 0;
}

static int mask(int how, const sigset_t *set, sigset_t *oldset,
	int (*real)(int, const sigset_t *, sigset_t *)) {
	if(config_noglobals())
		return real(how, set, oldset);

	bool segv_was_masked = segv_masked;
	sigset_t local;
	if(set && sigismember(set, SIGSEGV)) {
		if(how != SIG_UNBLOCK) {
			setup_segv_trampoline();
			memcpy(&local, set, sizeof local);
			sigdelset(&local, SIGSEGV);
			set = &local;
			segv_masked = true;
		} else if(segv_was_masked)
			segv_masked = false;
	}

	int res = real(how, set, oldset);
	if(oldset && segv_was_masked)
		sigaddset(oldset, SIGSEGV);
	return res;
}

#pragma weak libgotcha_sigprocmask = sigprocmask
int sigprocmask(int how, const sigset_t *set, sigset_t *oldset) {
	return mask(how, set, oldset, sigprocmask);
}

#pragma weak libgotcha_pthread_sigmask = pthread_sigmask
int pthread_sigmask(int how, const sigset_t *set, sigset_t *oldset) {
	return mask(how, set, oldset, pthread_sigmask);
}

#pragma weak libgotcha_sigfillset = sigfillset
int sigfillset(sigset_t *set) {
	int res = sigfillset(set);
	if(!config_noglobals())
		sigdelset(set, SIGSEGV);
	return res;
}

#pragma weak libgotcha_sigaddset = sigaddset
int sigaddset(sigset_t *set, int signum) {
	if(!config_noglobals() && signum == SIGSEGV)
		return 0;
	return sigaddset(set, signum);
}
