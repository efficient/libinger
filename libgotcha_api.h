// "Public" interface to the libgotcha GOT intercept library.
//
// Despite the marketing, this API isn't really for general consumption.  The library becomes active
// automatically and intercepts all subsequent inter--object file activity as soon as it is loaded
// into the process image.  This can happen either by virtue of some other object file in the
// application's dependency graph depending on libgotcha, or due to the inclusion of libgotcha or
// a dependent in LD_PRELOAD).  Statically linking the main binary against libgotcha is unsupported,
// as is loading it at runtime via the loader's dlopen() interface.
//
// Just having libgotcha active isn't actually good for anything unless you like needless overhead.
// That's where you come in: special *client libraries* can use this API to configure libgotcha for
// their own use case.  Such libraries (up to a limit of one per process image) may choose to
// *statically* link against libgotcha.  This confers two main advantages for this client: it
// becomes a singleton whose calls and accesses are always performed in the shared namespace, and
// any function definitions automatically shadow all other instances in the process image (but from
// an internal perspective, are shadowed by the next definition).  Note that a statically-linked
// client library is not subject to the interpositions imposed by libgotcha on the rest of the
// application; if this is not desired, see the supplementary libgotcha_repl.h header.

#ifndef LIBGOTCHA_H_
#define LIBGOTCHA_H_

#ifdef __cplusplus
extern "C" {
#endif

#include <stdint.h>

#define LIBGOTCHA_GROUP_ERROR -1
#define LIBGOTCHA_GROUP_SHARED 0

typedef long libgotcha_group_t;

static inline libgotcha_group_t libgotcha_group_thread_get(void) {
	libgotcha_group_t (*libgotcha_group_thread_accessor(void))(libgotcha_group_t);
	return libgotcha_group_thread_accessor()(LIBGOTCHA_GROUP_ERROR);
}

static inline libgotcha_group_t libgotcha_group_thread_set(libgotcha_group_t group) {
	libgotcha_group_t (*libgotcha_group_thread_accessor(void))(libgotcha_group_t);
	return libgotcha_group_thread_accessor()(group);
}

// Obtain a handle to an unallocated group, marking it as allocated.  Returns LIBGOTCHA_GROUP_ERROR
// if all groups are already allocated.  This and LIBGOTCHA_GROUP_SHARED are the only valid sources
// of group identifiers for use with libgotcha_group_thread_set().
libgotcha_group_t libgotcha_group_new(void);

void libgotcha_shared_hook(void (*)(void));

#ifdef __cplusplus
}
#endif

#endif
