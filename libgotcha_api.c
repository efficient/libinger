#include "libgotcha_api.h"

#include "namespace.h"
#include "shared.h"

#include <assert.h>
#include <stdatomic.h>

// Position N corresponds to namespace N+1!
static bool namespace_locked[NUM_SHADOW_NAMESPACES];

static libgotcha_group_t namespace_accessor(libgotcha_group_t new) {
	libgotcha_group_t *accessor = namespace_thread();
	libgotcha_group_t old = *accessor;
	if(new != LIBGOTCHA_GROUP_ERROR) {
		assert(new >= 0);
		assert(new <= NUM_SHADOW_NAMESPACES);
		assert(!new || namespace_locked[new - 1]);
		*accessor = new;
	}
	return old;
}

// We can't simply call namespace_thread() on the client code's behalf because the act of calling us
// would always cause a namespace switch, so we would always claim they had been executing in the
// base namespace.  Instead, we hand out a pointer and call an unexported function from outside.
libgotcha_group_t (*libgotcha_group_thread_accessor(void))(libgotcha_group_t) {
	return namespace_accessor;
}

static bool namespace_lock(libgotcha_group_t lmid) {
	assert(lmid > 0);
	assert(lmid <= NUM_SHADOW_NAMESPACES);

	bool unlocked = false;
	return atomic_compare_exchange_strong(namespace_locked + lmid - 1, &unlocked, !unlocked);
}

static void namespace_unlock(libgotcha_group_t lmid) {
	assert(lmid > 0);
	assert(lmid <= NUM_SHADOW_NAMESPACES);

	atomic_flag_clear(namespace_locked + lmid - 1);
}

libgotcha_group_t libgotcha_group_new(void) {
	(void) namespace_unlock;

	for(libgotcha_group_t chosen = 1; chosen <= NUM_SHADOW_NAMESPACES; ++chosen)
		if(namespace_lock(chosen))
			return chosen;

	return LIBGOTCHA_GROUP_ERROR;
}

void libgotcha_shared_hook(void (*hook)(void)) {
	shared_hook(hook);
}
