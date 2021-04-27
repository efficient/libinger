#include "ancillary.h"
#include "config.h"
#include "dynamic.h"
#include "globals.h"
#include "handle.h"
#include "handles.h"
#include "namespace.h"
#include "stack.h"
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

struct findsym_request {
	void *handle;
	const char *symbol;
	const char *version;
	const void *caller;
	void *res;
};

struct findsym_error {
	int code;
	const char *module;
	char *message;
	bool malloced;
	bool retrieved;
};

static thread_local struct findsym_error findsym_error;

static void findsym(struct findsym_request *req) {
	void *_dl_sym(void *, const char *, const void *);
	void *_dl_vsym(void *, const char *, const char *, const void *);
	if(req->version)
		req->res = _dl_vsym(req->handle, req->symbol, req->version, req->caller);
	else
		req->res = _dl_sym(req->handle, req->symbol, req->caller);
}

static void *findsym_interruptible(
	void *handle, const char *symbol, const char *version,
	bool interruptible, const void *caller
) {
	int _dl_catch_error(const char **, char **, bool *,
		void (*)(struct findsym_request *), struct findsym_request *);

	assert(caller);

	struct findsym_error *err = &findsym_error;
	if(err->malloced) {
		free(err->message);
		err->message = NULL;
	}
	err->retrieved = false;

	// The only reason that the dlsym() family ever mutates ld.so's state is to add an
	// inter-library dependency, which only happens if we search between libraries. So as long
	// as we aren't doing that, we can become interruptible now as far as ld.so is concerned.
	// Note, however, that we further exclude cases where we were called from shared code,
	// because it's still not okay to interrupt the *containing* noninterruptible sequence.
	// This case is impossible to detect using only the caller namespace, so we save(d) a flag.
	if(interruptible && handle != RTLD_DEFAULT && handle != RTLD_NEXT)
		*namespace_thread() = *namespace_caller();

	struct findsym_request req = {
		.handle = handle,
		.symbol = symbol,
		.version = version,
		.caller = caller,
	};
	if((err->code = _dl_catch_error(&err->module, &err->message, &err->malloced, findsym, &req)))
		return NULL;

	// No more interruption: we're about to take the PLOT lookup table's read lock!
	*namespace_thread() = 0;

	if(handle_is_plot_storage_ready()) {
		void *plot = (void *) handle_symbol_plot((uintptr_t) req.res);
		if(plot)
			req.res = plot;
	}
	err->message = NULL;
	return req.res;
}

#pragma weak libgotcha_dlsym = dlsym
void *dlsym(void *handle, const char *symbol) {
	if(dlsym == libgotcha_dlsym) {
		assert(!strcmp(symbol, "dlopen"));

		void *__libc_dlsym(const void *, const char *);
		const struct link_map *ldl = ancillary_loader();
		assert(ldl && "rr is unsupported for executables built w/ -znow (try -Wl,-zlazy)");
		return __libc_dlsym(ldl, symbol);
	}

	return findsym_interruptible(handle, symbol, NULL,
		stack_called_from_unshared(), stack_ret_addr_non_tramp());
}

#pragma weak libgotcha_dlvsym = dlvsym
void *dlvsym(void *handle, const char *symbol, const char *version) {
	return findsym_interruptible(handle, symbol, version,
		stack_called_from_unshared(), stack_ret_addr_non_tramp());
}

#pragma weak libgotcha_dlerror = dlerror
char *dlerror(void) {
	struct findsym_error *err = &findsym_error;
	if(err->message) {
		if(!err->retrieved) {
			struct findsym_error *err = &findsym_error;
			const char *sep = "";
			if(*err->module)
				sep = ": ";

			const char *descr = strerror(err->code);
			char *buf = malloc(
				strlen(err->module) + strlen(sep) +
				strlen(err->message) + 2 +
				strlen(descr) + 1
			);
			sprintf(buf, "%s%s%s: %s", err->module, sep, err->message, descr);
			if(err->malloced)
				free(err->message);
			err->message = buf;
			err->retrieved = true;
			return err->message;
		} else {
			free(err->message);
			err->message = NULL;
		}
	}

	return dlerror();
}

#pragma weak libgotcha_dlmopen = dlmopen
void *dlmopen(Lmid_t lmid, const char *filename, int flags) {
	if(lmid == LM_ID_BASE)
		return dlopen(filename, flags);

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

// This symbol must *not* be hidden, and so must not be declared in our header!
void *libgotcha_dl_open(
	const char *filename, int flags, uintptr_t caller, Lmid_t lmid,
	int argc, char **argv, char **env
) {
	if(findsym_error.malloced)
		free(findsym_error.message);
	findsym_error.message = NULL;
	return dynamic_open(filename, flags, caller, lmid, argc, argv, env);
}

// This symbol must *not* be hidden, and so must not be declared in our header!
void libgotcha_dl_close(void *handle) {
	if(findsym_error.malloced)
		free(findsym_error.message);
	findsym_error.message = NULL;
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
