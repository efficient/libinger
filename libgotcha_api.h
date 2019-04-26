#ifndef LIBGOTCHA_H_
#define LIBGOTCHA_H_

#include <stdint.h>

typedef long libgotcha_group_t;

libgotcha_group_t (*libgotcha_thread_group_getter(void))(void);

void libgotcha_shared_hook(void (*)(void));

#endif
