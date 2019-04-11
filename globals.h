#ifndef GLOBALS_H_
#define GLOBALS_H_

#include "error.h"

#include <signal.h>

enum error globals_init(void);
enum error globals_deinit(void);

void globals_install_handler(void (*)(int, siginfo_t *, void *));

#endif
