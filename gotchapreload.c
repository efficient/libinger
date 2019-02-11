#include "mirror_object.h"

#include <assert.h>
#include <dlfcn.h>
#include <stdbool.h>
#include <stdio.h>

static const char *(*const explainers[])(enum error) = {
	error_message,
	error_explanation,
	NULL,
};

static void __attribute__((constructor)) ctor(void) {
	enum error fail = mirror_object(dlopen(NULL, RTLD_LAZY), "");
	if(fail) {
		fputs("libgotchapreload.so", stderr);
		for(const char *(*const *thing_explainer)(enum error) = explainers; *thing_explainer; ++thing_explainer) {
			const char *thing_explanation = (*thing_explainer)(fail);
			if(thing_explanation)
				fprintf(stderr, ": %s", thing_explanation);
		}
		fputc('\n', stderr);
		assert(false);
	}
}

// If this weak symbol is not defined, the default constructor code in crti.o will invoke a dynamic
// symbol by the same name if it is non-NULL.  Unfortunately, since we wrap said symbol, it *will*
// be non-NULL even though the shadow GOT contains a NULL, resulting in a segfault when
// __libc_start_main() calls our _init() from within _start().
void __gmon_start__(void) {}
