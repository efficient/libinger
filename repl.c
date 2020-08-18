#include "repl.h"

#include "namespace.h"
#include "tcb.h"

#include <asm/prctl.h>
#include <assert.h>
#include <dlfcn.h>
#include <stddef.h>
#include <stdint.h>

#define STATIC_REPLACE(ret, fun, ...) \
	ret fun(__VA_ARGS__) { \
		static ret (*fun)(__VA_ARGS__) = NULL; \
		if(!fun) \
			*(void **) &fun = dlsym(RTLD_NEXT, #fun); \

STATIC_REPLACE(int, arch_prctl, int code, uintptr_t addr) //{
	Lmid_t nspc;
	uintptr_t prev = 0;
	const uintptr_t *before = NULL;
	if(code == ARCH_SET_FS) {
		nspc = *namespace_thread();
		before = tcb_parent();
		if(!*before) {
			int stat = arch_prctl(ARCH_GET_FS, (uintptr_t) &prev);
			if(stat)
				return stat;
		}
	}

	int stat = arch_prctl(code, addr);
	if(stat)
		return stat;

	if(code == ARCH_SET_FS) {
		uintptr_t *parent_tcb = tcb_parent();
		assert(parent_tcb != before);
		*namespace_thread() = nspc;
		*tcb_custom() = addr;
		if(prev)
			*parent_tcb = prev;
	}

	return stat;
}

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
