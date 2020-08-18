#include "repl.h"

#include "tcb.h"

#include <asm/prctl.h>
#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>

int arch_prctl(int, uintptr_t);

#define STATIC_REPLACE(ret, fun, ...) \
	ret fun(__VA_ARGS__) { \
		static ret (*fun)(__VA_ARGS__) = NULL; \
		if(!fun) \
			*(void **) &fun = dlsym(RTLD_NEXT, #fun); \

STATIC_REPLACE(void *, __tls_get_addr, uintptr_t index) //{
	if(!index)
		return NULL;

	uintptr_t restore = 0;
	uintptr_t parent_tcb = *tcb_parent();
	if(parent_tcb) {
		restore = *tcb_custom();
		arch_prctl(ARCH_SET_FS, parent_tcb);
	}

	void *res = __tls_get_addr(index);

	if(restore)
		arch_prctl(ARCH_SET_FS, restore);

	return res;
}

void repl_init(void) {
	__tls_get_addr(0);
}
