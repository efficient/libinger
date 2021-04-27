#ifndef ANCILLARY_H_
#define ANCILLARY_H_

#include "error.h"

#include <stdbool.h>

struct link_map;

bool ancillary_namespace(void);
const struct link_map *ancillary_loader(void);
enum error ancillary_disable_ctors_dtors(void);

#endif
