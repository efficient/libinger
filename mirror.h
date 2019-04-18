#ifndef MIRROR_H_
#define MIRROR_H_

#include "error.h"

struct link_map;

enum error mirror_object(const struct link_map *object, const char *optional_path);

#endif
