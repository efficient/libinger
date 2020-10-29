#include "ancillary.h"
#include "config.h"
#include "globals.h"
#include "handle.h"
#include "handles.h"
#include "interpose.h"
#include "namespace.h"
#include "repl.h"
#include "whitelist.h"

#include <sys/mman.h>
#include <assert.h>
#include <link.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <threads.h>

static struct link_map *root;

static enum error hook_object(struct handle *h, const struct link_map *l) {
	enum error code = handle_init(h, l, root);
	if(code)
		return code;

	return SUCCESS;
}

static inline enum error init(void) {
	// There can be only one!
	if(ancillary_namespace())
		// We don't want to initialize any copies of ourself that we may have loaded.
		return ancillary_disable_ctors_dtors();
	assert(namespace_self() && "libgotcha clash from already_bootstrapping() false negative");

	// Functions we interpose statically memoize the address of their external definitions when
	// first called.  This would be disastrous if the first call occurred in our signal handler,
	// so resolve the addresses eagerly instead where required.
	repl_init();

	// Start by rewriting our own GOT.  After this, any local calls to functions we interpose
	// will be routed to their external definitions.
	interpose_init();

	if(config_skip(handle_progname())) {
		fprintf(stderr, "libgotcha notice: Going dormant in skipped executable %s\n",
			handle_progname());
		return SUCCESS;
	}

	root = dlopen(NULL, RTLD_LAZY);
	if(!config_staticlink() && namespace_self() == root)
		// Eek!  Someone statically linked us into this executable.  Not cool: aside from
		// confining their code to run in the base namespace, that means we just gave them
		// an escape hatch from our interposed library functions!
		return ERROR_STATICALLY_LINKED;

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
	enum error code;
	if(!config_noglobals() && (code = globals_init()))
		return code;

	// Now multiplex everything and set up shadowing!
	return handles_shadow(root);
}

static inline void __attribute__((constructor(101))) ctor(void) {
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
