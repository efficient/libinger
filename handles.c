#include "handles.h"

#include "handle.h"
#include "namespace.h"

#include <assert.h>
#include <link.h>
#include <stddef.h>
#include <string.h>

static inline size_t find_lib(const char *const *deps, const char *soname) {
	const char *const *name;
	for(name = deps; *name && !strstr(*name, soname); ++name);
	return name - deps;
}

static enum error sort_deps(const char **deps, const struct link_map *lib) {
	enum error code;
	const struct handle *h = handle_get(lib, NULL, &code);
	if(!h)
		return code;
	else if(!handle_is_nodelete(h))
		return SUCCESS;

	const char *soname = strrchr(h->path, '/');
	soname = soname ? soname + 1 : h->path;
	if(deps[find_lib(deps, soname)])
		return SUCCESS;

	for(const ElfW(Dyn) *d = lib->l_ld; d->d_tag != DT_NULL; ++d)
		if(d->d_tag == DT_NEEDED) {
			const char *s = h->strtab + d->d_un.d_val;
			struct link_map *l = namespace_get(LM_ID_BASE, s, RTLD_LAZY);
			enum error bummer = sort_deps(deps, l);
			if(bummer)
				return bummer;
		}

	size_t index = find_lib(deps, soname);
	if(!deps[index])
		deps[index] = h->path;
	return SUCCESS;
}

static inline enum error sort_libs(const char **deps, const struct link_map *libs) {
	for(const struct link_map *l = libs; l; l = l->l_next) {
		enum error bummer = sort_deps(deps, l);
		if(bummer)
			return bummer;
	}
	return SUCCESS;
}

static inline size_t count_libs(const struct link_map *libs) {
	size_t count = 0;
	for(const struct link_map *l = libs; l; l = l->l_next)
		++count;
	return count;
}

// Create a new namespace, populating it with our own unmarked copies of any libraries that were
// initially marked with the NODELETE flag.  This way, further libraries loaded into this namespace
// will use our copies instead of the system ones to satisfy their dependencies.
static void nodelete_preshadow(const char *const *libs, Lmid_t namespace) {
	Lmid_t new = LM_ID_NEWLM;
	for(const char *const *lib = libs; *lib; ++lib) {
		struct link_map *l = dlmopen(new, *lib, RTLD_LAZY);
		assert(l);

		// Force the dynamic linker to consider this object when satisfying future
		// objects' dependencies.  This workaround is necessary to avoid bringing in
		// NODELETE objects that would prevent later namespace reinitialization.
		l->l_name += handle_nodelete_pathlen();
		new = namespace;
	}
}

// Fix the reference counts of any loaded libraries in the specified namespace that are marked
// NODELETE in the base namespace.  This way, a balanced number of future unloads from this
// namespace will result in its deinitialization.  Note, however, that any *new* dlmopen()'s in this
// namespace that occur after this call but before the next call to nodelete_preshadow() will
// indiscriminately use the system copies of dependencies, even if they are flagged as NODELETE.
static void nodelete_postshadow(const char *const *libs, Lmid_t namespace) {
	for(const char *const *lib = libs; *lib; ++lib) {
		struct link_map *l = namespace_get(namespace, *lib, RTLD_LAZY);
		assert(l);
		l->l_name -= handle_nodelete_pathlen();
		dlclose(l);
	}
}

enum error handles_shadow(const struct link_map *root) {
	const char *libs[count_libs(root)];
	memset(libs, 0, sizeof libs);

	enum error code = sort_libs(libs, root);
	if(code != SUCCESS)
		return code;

	for(Lmid_t n = 1; n <= NUM_SHADOW_NAMESPACES; ++n)
		nodelete_preshadow(libs, n);

	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code = handle_update(l, handle_got_shadow);
		if(code)
			return code;
	}

	for(Lmid_t n = 1; n <= NUM_SHADOW_NAMESPACES; ++n)
		nodelete_postshadow(libs, n);

	return SUCCESS;
}

bool handles_reshadow(const struct link_map *root, Lmid_t namespace) {
	const struct handle *bin = NULL;
	for(const struct link_map *l = root; l; l = l->l_next) {
		const struct handle *h = handle_get(l, NULL, NULL);
		assert(h);
		if(!bin)
			bin = h;

		if(h->owned) {
			struct link_map *n = namespace_get(namespace, h->path, RTLD_LAZY);
			assert(n);
			dlclose(n);
		}
	}

	// The namespace should now be empty (and nonexistent by the dynamic linker's definition)!
	assert(!namespace_get(namespace, bin->path, RTLD_LAZY) && dlerror());

	const char *libs[count_libs(root)];
	memset(libs, 0, sizeof libs);

	enum error code = sort_libs(libs, root);
	if(code != SUCCESS)
		return code;

	nodelete_preshadow(libs, namespace);
	for(const struct link_map *l = root; l; l = l->l_next)
		if(!handle_got_reshadow(handle_get(l, NULL, NULL), namespace))
			return false;
	nodelete_postshadow(libs, namespace);

	return true;
}
