#ifndef HANDLES_H_
#define HANDLES_H_

#include "error.h"

#include <dlfcn.h>
#include <stdbool.h>

struct link_map;

// Initialize all namespaces, populating each with the same libraries that are reachable from the
// provided library *root* (e.g., as obtained from dlopen(NULL, RTLD_LAZY)).
enum error handles_shadow(const struct link_map *);

// Restore the writeable (sub)segments of all libraries in the specified namespace and reachable
// from the given library *root*.  The source data is snapshots taken just after each library was
// initialized.  Also bumps the namespace's TLS version watermark, deferring TLS restoration.
bool handles_reshadow(const struct link_map *, Lmid_t);

// Restore the portions of the current TLS corresponding to the specified namespace.  The source
// data is the corresponding libraries' initialization images.  No op if the version is up to date.
void handles_restoretls(Lmid_t namespace);

#endif
