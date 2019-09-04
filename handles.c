#include "handles.h"

#include "handle.h"
#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <stddef.h>

// Create a new namespace, populating it with our own unmarked copies of any libraries that were
// initially marked with the NODELETE flag.  This way, further libraries loaded into this namespace
// will use our copies instead of the system ones to satisfy their dependencies.
static enum error nodelete_preshadow(const struct link_map *root, Lmid_t namespace) {
	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code;
		const struct handle *h = handle_get(l, NULL, &code);
		if(!h)
			return code;

		if(handle_is_nodelete(h)) {
			struct link_map *l = namespace_load(namespace, h->path, RTLD_LAZY);
			assert(l);

			// Force the dynamic linker to consider this object when satisfying future
			// objects' dependencies.  This workaround is necessary to avoid bringing in
			// NODELETE objects that would prevent later namespace reinitialization.
			l->l_name += handle_nodelete_pathlen();
		}
	}
	return SUCCESS;
}

// Fix the reference counts of any loaded libraries in the specified namespace that are marked
// NODELETE in the base namespace.  This way, a balanced number of future unloads from this
// namespace will result in its deinitialization.  Note, however, that any *new* dlmopen()'s in this
// namespace that occur after this call but before the next call to nodelete_preshadow() will
// indiscriminately use the system copies of dependencies, even if they are flagged as NODELETE.
static enum error nodelete_postshadow(const struct link_map *root, Lmid_t namespace) {
	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code;
		const struct handle *h = handle_get(l, NULL, &code);
		if(!h)
			return code;

		if(handle_is_nodelete(h)) {
			struct link_map *l = namespace_get(namespace, h->path, RTLD_LAZY);
			assert(l);
			l->l_name -= handle_nodelete_pathlen();
			dlclose(l);
		}
	}
	return SUCCESS;
}

enum error handles_shadow(const struct link_map *root) {
	for(Lmid_t n = 1; n <= NUM_SHADOW_NAMESPACES; ++n)
		nodelete_preshadow(root, n);

	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code = handle_update(l, handle_got_shadow);
		if(code)
			return code;
	}

	for(Lmid_t n = 1; n <= NUM_SHADOW_NAMESPACES; ++n)
		nodelete_postshadow(root, n);

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

	nodelete_preshadow(root, namespace);
	for(const struct link_map *l = root; l; l = l->l_next)
		if(!handle_got_reshadow(handle_get(l, NULL, NULL), namespace))
			return false;
	nodelete_postshadow(root, namespace);

	return true;
}
