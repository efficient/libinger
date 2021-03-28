#include "tcb.h"

#include <asm/prctl.h>
#include <assert.h>
#include <threads.h>

int arch_prctl(int, uintptr_t);

int tcb_prctl(int code, uintptr_t addr) {
	uintptr_t prev = 0;
	const uintptr_t *before = NULL;
	if(code == ARCH_SET_FS) {
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
