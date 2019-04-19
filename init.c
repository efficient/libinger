#include "globals.h"
#include "handle.h"
#include "interpose.h"
#include "namespace.h"
#include "threads.h"
#include "whitelist.h"

#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>

static struct link_map *root;

static inline bool in_ancillary_namespace(void) {
	return !namespace_self();
}

static inline enum error hook_object(struct handle *h, const struct link_map *l) {
	enum error code = handle_init(h, l, root);
	if(code)
		return code;

	return SUCCESS;
}

static inline enum error init(void) {
	// There can be only one!
	if(in_ancillary_namespace())
		// We don't want to initialize any copies of ourself that we may have loaded.
		return SUCCESS;

	// Start by rewriting our own GOT.  After this, any local calls to functions we interpose
	// will be routed to their external definitions.
	interpose_init();
	root = dlopen(NULL, RTLD_LAZY);

	// Initialize a handle for each object in the chain.
	// It's fine to do dependents before their dependencies here, so long as no thread that uses
	// a dependent installs a nonzero namespace selector before so doing.  But they shouldn't
	// be doing that from their constructors, anyway.
	enum error fresh = -1;
	for(const struct link_map *l = root;
		l && handle_get(l, hook_object, &fresh) && (signed) fresh <= SUCCESS;
		l = l->l_next);
	if((signed) fresh > SUCCESS)
		return fresh;

	// Populate the symbol whitelist, which determines which dynamic calls and accesses result
	// in a namespace switch.  And setup forced interposition, so that any calls to library
	// functions we define are routed to us instead.
	whitelist_shared_get(NULL);

	// Enable interception of cros--object file accesses to global storage.
	enum error code = globals_init();
	if(code)
		return code;

	// Now multiplex everything and set up shadowing!
	for(const struct link_map *l = root; l; l = l->l_next) {
		enum error code = handle_update(l, handle_got_shadow);
		if(code)
			return code;
	}

	return SUCCESS;
}

static inline void __attribute__((constructor)) ctor(void) {
	enum error fail = init();
	if(fail) {
		fprintf(stderr, "%s: libgotcha error: %s", handle_progname(), error_message(fail));
		const char *details = error_explanation(fail);
		if(details)
			fprintf(stderr, ": %s\n", details);
		else
			fputc('\n', stderr);
		exit(0xb1);
	}
}
