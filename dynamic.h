#ifndef DYNAMIC_H_
#define DYNAMIC_H_

#include <dlfcn.h>
#include <stdint.h>

#ifndef DYNAMIC_CONST
#define DYNAMIC_CONST const
#endif

void dynamic_init(void);

extern void *(*DYNAMIC_CONST dynamic_open)(const char *, int, uintptr_t, Lmid_t, int, char **, char **);
extern void (*DYNAMIC_CONST dynamic_close)(void *);

#endif
