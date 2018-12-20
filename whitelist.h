#ifndef WHITELIST_H_
#define WHITELIST_H_

#include <stdbool.h>

bool whitelist_copy_contains(const char *symbol);
bool whitelist_shared_contains(const char *symbol);
bool whitelist_so_contains(const char *path);

#endif
