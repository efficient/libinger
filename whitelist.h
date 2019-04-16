#ifndef WHITELIST_H_
#define WHITELIST_H_

#include <stdbool.h>
#include <stdint.h>

struct handle;

bool whitelist_copy_contains(const char *symbol);

bool whitelist_so_contains(const char *path);
void whitelist_so_insert(const struct handle *h);

// Check whether the symbol with the given name is whitelisted.  Rather than being multiplexed to
// the definitions in multiple copies of their defining shared object, whitelisted symbols always
// resolve to the definition from the base namespace, and using them results in a switch to this
// namespace if the relevant thread was not already operating in it.
// This function's return value can indicate one of three distinct scenarios:
//  * -1: This symbol is not whitelisted, and uses should be multiplexed to the current namespace.
//  *  0: This symbol is whitelisted, and uses should be redirected to the same object file's
//        definition in the base namespace.
//  * OW: This symbol is whitelisted and interposed, and uses should be redirected to the definition
//        at the returned address, which is usually our (*this* library's) own implementation.  Note
//        that this probably means we want to install the *trampoline* for said definition over the
//        (S)GOT entry, rather than simply the returned address!
//
// On the first call, populates the so (shared object) whitelist.
intptr_t whitelist_shared_get(const char *symbol);

#endif
