#include "namespace.h"

#include <asm/prctl.h>
#include <assert.h>
#include <dlfcn.h>
#include <stdint.h>
#include <threads.h>

#define STATIC_REPLACE(ret, fun, ...) \
	ret fun(__VA_ARGS__) { \
		static ret (*fun)(__VA_ARGS__) = NULL; \
		if(!fun) \
			*(void **) &fun = dlsym(RTLD_NEXT, #fun); \

// Unlike other TLS variables, these must *not* persist when the TCB is manually switched.  As such,
// they must be resolved via segment selector, not the __tls_get_addr() helper!
static thread_local uintptr_t custom_tcb;
static thread_local uintptr_t parent_tcb;

STATIC_REPLACE(int, arch_prctl, int code, uintptr_t addr) //{
	Lmid_t nspc;
	uintptr_t prev = 0;
	const uintptr_t *before = NULL;
	if(code == ARCH_SET_FS) {
		nspc = *namespace_thread();
		if(!parent_tcb) {
			int stat = arch_prctl(ARCH_GET_FS, (uintptr_t) &prev);
			if(stat)
				return stat;
			before = &parent_tcb;
		}
	}

	int stat = arch_prctl(code, addr);
	if(stat)
		return stat;

	if(code == ARCH_SET_FS) {
		assert(&parent_tcb != before);
		*namespace_thread() = nspc;
		custom_tcb = addr;
		if(prev)
			parent_tcb = prev;
	}

	return stat;
}

STATIC_REPLACE(void *, __tls_get_addr, uintptr_t index) //{
	if(!index)
		return NULL;

	uintptr_t restore = 0;
	if(parent_tcb) {
		restore = custom_tcb;
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
