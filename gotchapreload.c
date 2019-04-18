#include "mirror.h"

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

#define WRAPPER(ret, fun, ...) \
	static ret (*fun##_location(void))(__VA_ARGS__) { \
		static ret (*fun)(__VA_ARGS__); \
		static bool memoized; \
		if(!memoized) { \
			*(void **) &fun = dlsym(RTLD_NEXT, #fun); \
			memoized = true; \
		} \
		return fun; \
	}
