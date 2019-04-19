#ifndef GLOBALS_H_
#define GLOBALS_H_

#include "error.h"

#include <signal.h>

enum error globals_init(void);
enum error globals_deinit(void);

struct sigaction *globals_handler(void);

#endif
