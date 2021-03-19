#include "libinger.h"

#include <stdio.h>
#include <string.h>

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
