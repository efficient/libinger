#include "ancillary.h"
#include "config.h"
#include "globals.h"
#include "handle.h"
#include "handles.h"
#include "interpose.h"
#include "namespace.h"
#include "whitelist.h"

#include <sys/mman.h>
#include <assert.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <threads.h>

static struct link_map *root;

static unsigned addr_width(void) {
	unsigned word_size = sizeof &addr_width * 8;
	for(unsigned bit = word_size - 1; bit; --bit)
		if(((uintptr_t) addr_width) >> bit == 0x1)
			return bit;
	return word_size;
}

static bool in_ancillary_namespace(void) {
	// Should resolve to the executable's .text section, which the kernel and dynamic linker
	// load at wildly different addresses.  We'll compare against our own address to decide
	// which one loaded the current namespace's copy of the executable.
	#pragma weak _start
	void _start(void);

	if(!_start) {
		if(strstr(getenv("LD_PRELOAD"), namespace_self()->l_name))
			// We've been preloaded.  It's safe to skip the check (as long as the
			// executable doesn't *also* depend on us, even transitively): we won't load
			// duplicate copies of ourselves into the ancillary namespaces.
			return false;

		fputs("libgotcha error: missing _start symbol (rebuild executable?)\n", stderr);
		abort();
	}

	uintptr_t mask = 0x3ul << (addr_width() - 1);
	return ((uintptr_t) _start & mask) == ((uintptr_t) in_ancillary_namespace & mask);
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
		return ancillary_disable_ctors_dtors();
	assert(namespace_self() && "libgotcha clash from in_ancillary_namespace() false negative");

	// Start by rewriting our own GOT.  After this, any local calls to functions we interpose
	// will be routed to their external definitions.
	interpose_init();
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
