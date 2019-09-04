#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <pthread.h>
#include <threads.h>

Lmid_t *namespace_thread(void) {
	static thread_local Lmid_t namespace = LM_ID_BASE;
	return &namespace;
}

bool *namespace_thread_tramp(void) {
	static thread_local bool trampolining;
	return &trampolining;
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
