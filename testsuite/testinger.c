#include "libinger.h"

#include <errno.h>
#include <stdio.h>
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

#pragma weak libtestinger_getenv = getenv
const char *getenv(const char *name) {
	(void) name;
	return "libtestinger.so";
}

#pragma weak libtestinger_libc_start_main = __libc_start_main
int __libc_start_main(int (*main)(int, char **, char **), int argc, char**argv, int (*init)(int, char **, char **), void (*fini)(void), void (*rtld_fini)(void), void *stack_end) {
	const char *skiplist = getenv("LIBGOTCHA_SKIP");
	if(skiplist && strstr(skiplist, *argv))
		return __libc_start_main(main, argc, argv, init, fini, rtld_fini, stack_end);

	if(getenv("LIBTESTINGER_VERBOSE")) {
		fputs("!!! LD_PRELOAD=.../libtestinger.so", stderr);
		for(char **arg = argv; arg != argv + argc; ++arg)
			fprintf(stderr, " %s", *arg);
		fputc('\n', stderr);
	}

	mainfunc = main;
	return __libc_start_main(testinger, argc, argv, init, fini, rtld_fini, stack_end);
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
