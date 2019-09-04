#include "handles.h"

#include "handle.h"
#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <stddef.h>

enum error handles_shadow(const struct link_map *root) {
	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code = handle_update(l, handle_got_shadow);
		if(code)
			return code;
	}

	return SUCCESS;
}

bool handles_reshadow(const struct link_map *root, Lmid_t namespace) {
	for(const struct link_map *l = root; l; l = l->l_next) {
		const struct handle *h = handle_get(l, NULL, NULL);
		assert(h);
		if(h->owned) {
			struct link_map *n = namespace_get(namespace, h->path, RTLD_LAZY);
			assert(n);
			dlclose(n);
		}
	}

	for(const struct link_map *l = root; l; l = l->l_next)
		if(!handle_got_reshadow(handle_get(l, NULL, NULL), namespace))
			return false;
	return true;
}
