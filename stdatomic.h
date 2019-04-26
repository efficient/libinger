#ifndef STDATOMIC_H_
#define STDATOMIC_H_

#ifndef __STDC_NO_ATOMICS__
// Defer to the system header.  This works as long as there's no sanitizer/ folder in our tree.
# include <sanitizer/../stdatomic.h>
#else
enum memory_order {
	memory_order_relaxed = __ATOMIC_RELAXED,
	memory_order_consume = __ATOMIC_CONSUME,
	memory_order_acquire = __ATOMIC_ACQUIRE,
	memory_order_release = __ATOMIC_RELEASE,
	memory_order_acq_rel = __ATOMIC_ACQ_REL,
	memory_order_seq_cst = __ATOMIC_SEQ_CST,
};

#define atomic_compare_exchange_strong(object, expected, desired) \
	atomic_compare_exchange_strong_explicit(object, expected, desired, memory_order_acquire, memory_order_relaxed)
#define atomic_compare_exchange_strong_explicit(object, expected, desired, success, failure) \
	__atomic_compare_exchange_n(object, expected, desired, true, success, failure)

#define atomic_flag_clear(object) \
	atomic_flag_clear_explicit(object, memory_order_release)
#define atomic_flag_clear_explicit(object, order) \
	__atomic_clear(object, order)
#endif

#endif
