#include "repl.h"

#include "tcb.h"

#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>

int arch_prctl(int, uintptr_t);

#define STATIC_REPLACE(ret, fun, ...) \
	ret fun(__VA_ARGS__) { \
		static ret (*fun)(__VA_ARGS__) = NULL; \
		if(!fun) \
			*(void **) &fun = dlsym(RTLD_NEXT, #fun); \

STATIC_REPLACE(void *, __tls_get_addr, struct tls_symbol *symb) //{
	if(!symb)
		return NULL;

	uintptr_t parent = 0;
	if(symb->index >= 0)
		parent = *tcb_parent();
	else
		symb->index = -symb->index - 1;

	void *res = __tls_get_addr(symb);
	if(parent) {
		uintptr_t offset = *tcb_custom() - (uintptr_t) res;
		res = (void *) (parent - offset);
	}

	return res;
}

void repl_init(void) {
	__tls_get_addr(NULL);
}
