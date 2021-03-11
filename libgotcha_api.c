#include "libgotcha_api.h"

#include "config.h"
#include "handle.h"
#include "handles.h"
#include "namespace.h"
#include "shared.h"

#include <assert.h>
#include <link.h>
#include <stdatomic.h>
#include <stddef.h>
#include <stdint.h>

void *__tls_get_addr(uintptr_t);

// Position N corresponds to namespace N+1!
static bool namespace_locked[NUM_SHADOW_NAMESPACES];

static libgotcha_group_t namespace_accessor(libgotcha_group_t new) {
	libgotcha_group_t *accessor = namespace_thread();
	libgotcha_group_t old = *accessor;
	if(new != LIBGOTCHA_GROUP_ERROR) {
		assert(new >= 0);
		assert(new <= config_numgroups());
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
	assert(lmid <= config_numgroups());

	bool unlocked = false;
	return atomic_compare_exchange_strong(namespace_locked + lmid - 1, &unlocked, !unlocked);
}

static void namespace_unlock(libgotcha_group_t lmid) {
	assert(lmid > 0);
	assert(lmid <= config_numgroups());

	atomic_flag_clear(namespace_locked + lmid - 1);
}

libgotcha_group_t libgotcha_group_new(void) {
	(void) namespace_unlock;

	for(libgotcha_group_t chosen = 1; chosen <= config_numgroups(); ++chosen)
		if(namespace_lock(chosen))
			return chosen;

	return LIBGOTCHA_GROUP_ERROR;
}

bool libgotcha_group_renew(libgotcha_group_t which) {
	return handles_reshadow(dlopen(NULL, RTLD_LAZY), which);
}

size_t libgotcha_group_limit(void) {
	return config_numgroups();
}

void *libgotcha_group_symbol(libgotcha_group_t which, const char *symbol) {
	assert(symbol);

	struct link_map *l = dlopen(NULL, RTLD_LAZY);
	if(which) {
		l = namespace_get(which, handle_progname(), RTLD_LAZY);
		if(!l)
			return NULL;
	}
	return dlsym(l, symbol);
}

void libgotcha_shared_hook(void (*hook)(void)) {
	shared_hook(hook);
}

// The following definitions permit a client libraries to call the wrapper functions associated with
// our static interpositions, similar to how a statically-linked client library might do with
// dynamic interpositions via the libgotcha_repl.h interface.

void *libgotcha_tls_get_addr(uintptr_t index) {
	return __tls_get_addr(index);
}
