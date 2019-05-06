#include "config.h"
#include "globals.h"
#include "namespace.h"

#include <assert.h>
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
