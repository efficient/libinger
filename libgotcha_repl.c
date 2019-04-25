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

#pragma weak libgotcha_dlmopen = dlmopen
void *dlmopen(Lmid_t lmid, const char *filename, int flags) {
	if(lmid == LM_ID_BASE)
		return dlmopen(lmid, filename, flags);

	// Trick dlerror() into reporting "invalid target namespace for dlmopen()."
	void *null = dlmopen(NUM_SHADOW_NAMESPACES + 1, "", RTLD_LAZY);
	assert(!null);
	return null;
}

static bool segv_masked;
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
	if(signum != SIGSEGV || config_noglobals())
		return sigaction(signum, act, oldact);

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

#pragma weak libgotcha_sigprocmask = sigprocmask
int sigprocmask(int how, const sigset_t *set, sigset_t *oldset) {
	if(config_noglobals())
		return sigprocmask(how, set, oldset);

	bool segv_was_masked = segv_masked;
	sigset_t local;
	if(set && how != SIG_UNBLOCK && sigismember(set, SIGSEGV)) {
		setup_segv_trampoline();
		memcpy(&local, set, sizeof local);
		sigdelset(&local, SIGSEGV);
		set = &local;
		segv_masked = true;
	}

	int res = sigprocmask(how, set, oldset);
	if(oldset && segv_was_masked)
		sigaddset(oldset, SIGSEGV);
	return res;
}
