// Function replacement interface for the libgotcha GOT intercept library.
//
// You don't need this unless you're writing a statically-linked client library that needs to call
// one of libgotcha's library function wrappers.  By default, such library's calls are retargeted at
// the interposed external definition, but this behavior can be overridden by making your calls with
// the special names declared herein.  Note that you will first need to expose the symbols, e.g. by
// invoking objcopy --globalize-symbol on the libgotcha static library.
//
// This header may be especially useful to client libraries that need to interpose on the same
// symbols as libgotcha, as it allows them to defer to the libgotcha implementations when
// appropriate.  In order to override the symbols, it is first necessary to weaken the corresponding
// library symbols, e.g. by invoking objcopy -W on the libgotcha static library.
//
// If a client library wants to interpose a symbol not already wrapped by libgotcha, or wants to
// interpose one but not provide access to libgotcha's own implementation---danger, W.R.!---it
// doesn't need this header at all: instead, it should just make what appears to be a recursive
// call.  To prevent the C compiler from optimizing away such a call, the containing module should
// be compiled with the -fno-optimize-sibling-calls switch and *without* the -fpic or -fPIC one.  If
// the latter is not possible, it may be necessary to create an alias of the wrapping definition,
// e.g. using an attribute or pragma.

#ifndef LIBGOTCHA_REPL_H_
#define LIBGOTCHA_REPL_H_

#ifdef __cplusplus
extern "C" {
#endif

#include <dlfcn.h>
#include <features.h>
#include <stdbool.h>
#include <stdint.h>

// Special build-time configuration flag.  If a program defines this symbol and sets it to true,
// statically linking its executable against libgotcha will be permitted.  Note that, in this case,
// libgotcha's functionality will only be accessible to third-party shared libraries.
extern const bool libgotcha_staticlink;

void *libgotcha_dlsym(void *, const char *);
char *libgotcha_dlerror(void);

#ifdef __USE_GNU
void *libgotcha_dlvsym(void *, const char *, const char *);
void *libgotcha_dlmopen(Lmid_t, const char *, int);

struct dl_phdr_info;
int libgotcha_dl_iterate_phdr(int (*callback)(struct dl_phdr_info *, size_t, void *), void *data);
#endif

int libgotcha_arch_prctl(int, uintptr_t);

#ifdef __USE_POSIX
#include <signal.h>

int libgotcha_pthread_kill(pthread_t, int);

int libgotcha_sigaction(int, const struct sigaction *, struct sigaction *);

int libgotcha_sigprocmask(int, const sigset_t *, sigset_t *);
int libgotcha_pthread_sigmask(int, const sigset_t *, sigset_t *);

int libgotcha_sigfillset(sigset_t *);
int libgotcha_sigaddset(sigset_t *, int);
#endif

void (*libgotcha_signal(int, void (*)(int)))(int);

#ifdef __cplusplus
}
#endif

#endif
