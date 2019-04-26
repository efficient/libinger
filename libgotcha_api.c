#include "libgotcha_api.h"

#include "namespace.h"
#include "shared.h"

static libgotcha_group_t namespace_getter(void) {
	return *namespace_thread();
}

// We can't simply call namespace_thread() on the client code's behalf because the act of calling us
// would always cause a namespace switch, so we would always claim they had been executing in the
// base namespace.  Instead, we hand out a pointer and call an unexported function from outside.
libgotcha_group_t (*libgotcha_group_thread_getter(void))(void) {
	return namespace_getter;
}

void libgotcha_shared_hook(void (*hook)(void)) {
	shared_hook(hook);
}
