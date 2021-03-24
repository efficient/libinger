#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <pthread.h>
#include <threads.h>

// NB: Unlike other modules, the TLS variables defined herein only persist across TCB switches if
//     explicitly propagated during a call to arch_prctl().

Lmid_t *namespace_thread(void) {
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
