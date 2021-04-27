#include "libgotcha.h"
#include "libgotcha_repl.h"
#include "libinger.h"

#include <sys/types.h>
#include <assert.h>
#include <dlfcn.h>
#include <errno.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>

struct inout {
	int argc;
	char **argv;
	char **envp;
	int retval;
};

static int (*mainfunc)(int, char **, char **);

static void testinging(void *inout) {
	struct inout *argret = inout;
	argret->retval = mainfunc(argret->argc, argret->argv, argret->envp);
}

static int testinger(int argc, char **argv, char **envp) {
	struct inout argret = {
		.argc = argc,
		.argv = argv,
		.envp = envp,
	};
	launch(testinging, UINT64_MAX, &argret);
	return argret.retval;
}

#pragma weak libtestinger_libc_start_main = __libc_start_main
int __libc_start_main(int (*main)(int, char **, char **), int argc, char**argv, int (*init)(int, char **, char **), void (*fini)(void), void (*rtld_fini)(void), void *stack_end) {
	const char *skiplist = getenv("LIBGOTCHA_SKIP");
	if(skiplist && strstr(skiplist, *argv))
		return __libc_start_main(main, argc, argv, init, fini, rtld_fini, stack_end);

	mainfunc = main;
	return __libc_start_main(testinger, argc, argv, init, fini, rtld_fini, stack_end);
}

#pragma weak libtestinger_signal = signal
void (*signal(int signum, void (*handler)(int)))(int) {
	if(handler == SIG_DFL)
		return handler;

	return signal(signum, handler);
}

static bool intrsleep;

#pragma weak libtestinger_alarm = alarm
unsigned int alarm(unsigned int seconds) {
	intrsleep = true;
	return alarm(seconds);
}

#pragma weak libtestinger_nanosleep = nanosleep
int nanosleep(const struct timespec *req, struct timespec *rem) {
	struct timespec ours;
	if(!rem)
		rem = &ours;

	int stat = nanosleep(req, rem);
	if(intrsleep) {
		intrsleep = false;
		return stat;
	}
	while(stat && errno == EINTR) {
		struct timespec next;
		memcpy(&next, rem, sizeof next);
		stat = nanosleep(&next, rem);
	}
	return stat;
}

int libtestinger_nanosleep(const struct timespec *, struct timespec *);

#pragma weak libtestinger_usleep = usleep
int usleep(useconds_t usec) {
	struct timespec nsec = {
		.tv_sec = usec / 1000000,
		.tv_nsec = (usec % 1000000) * 1000,
	};
	return libtestinger_nanosleep(&nsec, NULL);
}

void _dl_signal_error(int, const char *, const char *, const char *);
void libtestinger_dl_signal_exception(int error, const char *const *module, const char *message);

extern const char *__progname;

// It seems that glibc has a bug: __libc_dlsym() calls from ancillary namespaces abort the process
// if they cannot find the target symbol, even if they would ordinarily only return an error code!
// This happens because _dl_signal_cexception() calls are always redirected back to the base
// namespace, so we work around it by proxying them and redirecting to the correct namespace.
#pragma weak libtestinger_dl_signal_exception = _dl_signal_exception
void _dl_signal_exception(int error, const char *const *module, const char *message) {
	// If we're still bootstrapping dlsym(), just jump back to libc however we can get there!
	if(dlsym == libgotcha_dlsym) {
		if(_dl_signal_exception == libtestinger_dl_signal_exception)
			_dl_signal_error(error, module[0], message, module[1]);
		else
			_dl_signal_exception(error, module, message);
	}

	// Otherwise, jump to the copy of libc in the namespace we came from.
	libgotcha_group_t group = libgotcha_group_caller();
	void (*_dl_signal_exception)(int, const char *const *, const char *) =
		(void (*)(int, const char *const *, const char *)) (uintptr_t)
			libgotcha_group_symbol_from(group, "_dl_signal_exception", "libc.so.6");
	libgotcha_group_thread_set(group);
	assert(_dl_signal_exception);
	assert(_dl_signal_exception != libtestinger_dl_signal_exception);
	if(mainfunc)
		fprintf(stderr, "./%s: symbol lookup warning: %s: %s (code %d)\n", __progname, *module, message, error);
	_dl_signal_exception(error, module, message);
}
