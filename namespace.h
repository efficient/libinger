#ifndef NAMESPACE_H_
#define NAMESPACE_H_

#include <dlfcn.h>
#include <stdbool.h>

#define NUM_SHADOW_NAMESPACES 15

struct link_map;

typedef unsigned version_t;

// Guarantees its *only* side effect is to clobber the return register.
Lmid_t *namespace_thread(void);

// Guarantees its *only* side effect is to clobber the return register.
Lmid_t *namespace_caller(void);

// Check whether the current namespace is currently executing the shared-code completion trampoline.
bool *namespace_trampolining(Lmid_t);

// Retrieve the global TLS version watermark for the specified namespace.
version_t *namespace_curversion(Lmid_t);

// Retrieve *this* TLS's version number for the specified namespace.  Comparing it against the
// contents of namespace_currentversion() for the same namespace indicates whether the corresponding
// portion of the TLS area needs to be restored from the initialization image before use.
version_t *namespace_tlsversion(Lmid_t);

// Returns the our own handle only if we're loaded in the base namespace (including LD_PRELOADs).
const struct link_map *namespace_self(void);

// This function MUST NOT be called on the dynamic linker itself: the reference counting of its
// link_map works differently than that of other object files', and it reacts VERY poorly to being
// dlclose()'d!
struct link_map *namespace_get(Lmid_t lmid, const char *filename, int flags);

#endif
