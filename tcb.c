#include "tcb.h"

#include "namespace.h"

#include <asm/prctl.h>
#include <assert.h>
#include <dlfcn.h>
#include <threads.h>

int arch_prctl(int, uintptr_t);

int tcb_prctl(int code, uintptr_t addr) {
	// If we're swapping out the TCB, we need to copy over our record of our caller's namespace.
	// No need to do the current namespace, as that is always zero here and will get
	// automagically updated from this when we return back to the caller.  See the
	// implementation note in the namespace module for more details.
	Lmid_t caller = 0;
	uintptr_t prev = 0;
	const uintptr_t *before = NULL;
	if(code == ARCH_SET_FS) {
		caller = *namespace_caller();
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
		*namespace_caller() = caller;
		*tcb_custom() = addr;
		if(prev)
			*parent_tcb = prev;
	}

	return stat;
}

uintptr_t *tcb_custom(void) {
	static thread_local uintptr_t custom_tcb;
	return &custom_tcb;
}

uintptr_t *tcb_parent(void) {
	static thread_local uintptr_t parent_tcb;
	return &parent_tcb;
}
