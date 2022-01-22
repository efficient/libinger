#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <pthread.h>
#include <threads.h>

// NB: Unlike other modules, the thread_local variables defined herein do not persist across TCB
//     switches.  The backing stores for namespace_{caller,thread}() are "exceptions", for two
//     different reasons:
//      * The tcb module manually copies over the caller namespace whenever a new TCB is installed.
//      * Since we only run our own code in the shared namespace and all manual TCB switches occur
//        within our code, the PLOT trampoline will always restore the *installed* TCB's current
//        namespace from its caller namespace on its way back out of any call into us from a
//        non-shared namespace.
//
//     One weird case is the namespace_trampolining() backing store: though declared as a global, it
//     is actually an emulated TCB-agnostic thread-local variable, which is necessary because we
//     cannot affort __tls_get_addr() calls from the injected procedure_linkage_override() code.
//     (If we ever want to support pthread_create() or clone() calls from a non-shared namespace,
//     we'll either need to revisit the latter design or assign a new namespace to the child;
//     otherwise, the control library loses the assurance that its trampoline hook will run.)

Lmid_t *namespace_thread(void) {
	static thread_local Lmid_t namespace = LM_ID_BASE;
	return &namespace;
}

Lmid_t *namespace_caller(void) {
	static thread_local Lmid_t namespace = LM_ID_BASE;
	return &namespace;
}

bool *namespace_trampolining(Lmid_t optional) {
	static bool trampolining[NUM_SHADOW_NAMESPACES];
	Lmid_t namespace = optional ? optional : *namespace_thread();
	assert(namespace);
	return trampolining + namespace - 1;
}

version_t *namespace_curversion(Lmid_t required) {
	static version_t versions[NUM_SHADOW_NAMESPACES];
	assert(required);
	return versions + required - 1;
}

version_t *namespace_tlsversion(Lmid_t required) {
	static thread_local version_t versions[NUM_SHADOW_NAMESPACES];
	assert(required);
	return versions + required - 1;
}

const struct link_map *namespace_self(void) {
	static const struct link_map *memo;
	if(!memo)
		for(const struct link_map *l = dlopen(NULL, RTLD_LAZY); l; l = l->l_next)
			if(l->l_ld == _DYNAMIC)
				return memo = l;
	return memo;
}

struct link_map *namespace_get(Lmid_t lmid, const char *filename, int flags) {
	struct link_map *l = dlmopen(lmid, filename, flags | RTLD_NOLOAD);
	if(l)
		dlclose(l);
	return l;
}
