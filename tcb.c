#include "tcb.h"

#include <threads.h>

uintptr_t *tcb_custom(void) {
	static thread_local uintptr_t custom_tcb;
	return &custom_tcb;
}

uintptr_t *tcb_parent(void) {
	static thread_local uintptr_t parent_tcb;
	return &parent_tcb;
}
