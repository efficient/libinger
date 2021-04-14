#ifndef STACK_H_
#define STACK_H_

#include <stdbool.h>

static inline const void *stack_ret_addr(void) {
	return __builtin_extract_return_addr(__builtin_return_address(0));
}

static inline const void **stack_base_addr(void) {
	return __builtin_frame_address(0);
}

static inline bool stack_is_trampoline(const void *addr) {
	bool plot_is_trampoline(const void *);
	return plot_is_trampoline(addr);
}

// Call this directly from the body of a dynamic replacement function.
static inline bool stack_called_from_unshared(void) {
	return stack_is_trampoline(stack_ret_addr());
}

// Call this directly from the body of a dynamic replacement function.
static inline const void *stack_ret_addr_non_tramp(void) {
	// Walking the stack in this manner only works assuming that there are exactly two
	// trampoline functions and that neither keeps anything else in its stack frame.
	const void **ras = stack_base_addr() + 1;
	for(const void **it = ras; it != ras + 3; ++it)
		if(!stack_is_trampoline(*it))
			return *it;

	// If we ever get here, the trampolines recursed.  Yikes!
	return NULL;
}

#endif
