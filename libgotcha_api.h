#ifndef LIBGOTCHA_H_
#define LIBGOTCHA_H_

#include <stdint.h>

typedef long libgotcha_group_t;

static inline libgotcha_group_t libgotcha_group_thread(void) {
	libgotcha_group_t (*libgotcha_group_thread_getter(void))(void);
	return libgotcha_group_thread_getter()();
}

void libgotcha_shared_hook(void (*)(void));

#endif
