#include "libgotcha_api.h"

#include "namespace.h"
#include "shared.h"

static libgotcha_group_t namespace_getter(void) {
	return *namespace_thread();
}

libgotcha_group_t (*libgotcha_thread_group_getter(void))(void) {
	return namespace_getter;
}

void libgotcha_shared_hook(void (*hook)(void)) {
	shared_hook(hook);
}
