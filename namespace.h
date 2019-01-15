#ifndef NAMESPACE_H_
#define NAMESPACE_H_

#include <dlfcn.h>

#define NUM_SHADOW_NAMESPACES 2

struct link_map;

Lmid_t *namespace_thread(void);
struct link_map *namespace_load(Lmid_t lmid, const char *filename, int flags);

#endif
