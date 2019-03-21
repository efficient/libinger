#ifndef WHITELIST_H_
#define WHITELIST_H_

#include <stdbool.h>

struct handle;

bool whitelist_copy_contains(const char *symbol);
bool whitelist_so_contains(const char *path);
void whitelist_so_insert(const struct handle *h);
bool whitelist_shared_contains(const char *symbol);

#endif
