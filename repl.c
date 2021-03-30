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

STATIC_REPLACE(void *, __tls_get_addr, struct tls_symbol *symb) //{
	if(!symb)
		return NULL;

	uintptr_t restore = 0;
	if(symb->index >= 0) {
		uintptr_t parent_tcb = *tcb_parent();
		if(parent_tcb) {
			restore = *tcb_custom();
			arch_prctl(ARCH_SET_FS, parent_tcb);
		}
	} else
		symb->index = -symb->index - 1;

	void *res = __tls_get_addr(symb);

	if(restore)
		arch_prctl(ARCH_SET_FS, restore);

	return res;
}

void repl_init(void) {
	__tls_get_addr(NULL);
}
