#include "mirror_object.h"

#include "globals.h"
#include "handle.h"
#include "threads.h"
#include "whitelist.h"

#include <assert.h>
#include <link.h>
#include <string.h>

static thread_local const struct link_map *mirroring;

static enum error hook_object(struct handle *h, const struct link_map *l) {
	assert(mirroring);
	enum error code = handle_init(h, l, (struct link_map *) mirroring);
	if(code)
		return code;

	return SUCCESS;
}

static bool in_ancillary_namespace(void) {
	static bool ancillary;
	static bool memoized;
	if(!memoized) {
		for(const struct link_map *l = dlopen(NULL, RTLD_LAZY); l->l_ld != _DYNAMIC; l = l->l_next)
			if(!l->l_next) {
				ancillary = true;
				break;
			}
		memoized = true;
	}
	return ancillary;
}

enum error mirror_object(const struct link_map *lib, const char *fname) {
	assert(lib);
	assert(fname);
	if(lib->l_name && *lib->l_name && strcmp(lib->l_name, fname))
		return ERROR_FNAME_MISMATCH;

	// There can be only one!
	if(in_ancillary_namespace())
		return SUCCESS;

	// Initialize a handle for each object in the chain.
	// It's fine to do dependents before their dependencies here, so long as no thread that uses
	// a dependent installs a nonzero namespace selector before so doing. But that's impossible
	// because our public API should force it to first call into here and block!
	enum error fresh = -1;
	mirroring = lib;
	for(const struct link_map *l = lib;
		l && handle_get(l, hook_object, &fresh) && (signed) fresh <= SUCCESS;
		l = l->l_next);
	mirroring = NULL;
	if((signed) fresh > SUCCESS)
		return fresh;

	// Populate the symbol whitelist.
	// TODO: Make whitelist_shared_init() detect and implicitly whitelist us using a generalized
	//       version of in_ancillary_namespace(); this way, any client code calls to our public
	//       API will be automagically proxied back to the base namespace and preemption will be
	//       preempted while we're in here.
	whitelist_shared_get(NULL);

	// Enable interception of cross--object file accesses to global storage.
	enum error code = globals_init();
	if(code)
		return code;

	// Now multiplex everything and set up shadowing!
	for(const struct link_map *l = lib; l; l = l->l_next) {
		enum error code = handle_update(l, handle_got_shadow);
		if(code)
			return code;
	}

	return SUCCESS;
}
