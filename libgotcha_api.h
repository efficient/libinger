#ifndef LIBGOTCHA_H_
#define LIBGOTCHA_H_

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

#endif
