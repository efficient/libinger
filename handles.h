#ifndef HANDLES_H_
#define HANDLES_H_

#include "error.h"

#include <dlfcn.h>
#include <stdbool.h>

struct link_map;

// Initialize all namespaces, populating each with the same libraries that are reachable from the
// provided library *root* (e.g., as obtained from dlopen(NULL, RTLD_LAZY)).
enum error handles_shadow(const struct link_map *);

// Deinitialize then reinitialize the specified namespace from the given library *root*.
bool handles_reshadow(const struct link_map *, Lmid_t);

#endif
