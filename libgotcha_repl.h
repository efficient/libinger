#ifndef LIBGOTCHA_REPL_H_
#define LIBGOTCHA_REPL_H_

#ifdef __cplusplus
extern "C" {
#endif

#include <features.h>

#ifdef __USE_GNU
#include <dlfcn.h>

void *libgotcha_dlmopen(Lmid_t, const char *, int);
#endif

#ifdef __USE_POSIX
#include <signal.h>

int libgotcha_sigaction(int, const struct sigaction *, struct sigaction *);

int libgotcha_sigprocmask(int, const sigset_t *, sigset_t *);
#endif

void (*libgotcha_signal(int, void (*)(int)))(int);

#ifdef __cplusplus
}
#endif

#endif
