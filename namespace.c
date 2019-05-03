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

struct link_map *namespace_load(Lmid_t lmid, const char *filename, int flags) {
	assert(!(flags & RTLD_NOLOAD));

	static Lmid_t fast = LM_ID_BASE;
	static pthread_mutex_t slow = PTHREAD_MUTEX_INITIALIZER;

	if(lmid <= fast)
		return dlmopen(lmid, filename, flags);

	assert(lmid == fast + 1 && "namespaces must be initialized in order");
	pthread_mutex_lock(&slow);

	struct link_map *l = dlmopen(lmid > fast ? LM_ID_NEWLM : lmid, filename, flags);
	Lmid_t check = LM_ID_NEWLM;
	if(l) {
		dlinfo(l, RTLD_DI_LMID, &check);
		assert(check == lmid);
		fast = lmid;
	}

	pthread_mutex_unlock(&slow);
	return l;
}

const struct link_map *namespace_get(Lmid_t lmid, const char *filename, int flags) {
	struct link_map *l = dlmopen(lmid, filename, flags | RTLD_NOLOAD);
	if(l)
		dlclose(l);
	return l;
}
