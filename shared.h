#ifndef SHARED_H_
#define SHARED_H_

#include <stdbool.h>

// The provided callback is invoked once whenever switching from shared code to a private copy.  The
// provided function must not perform any floating-point calculations!
void shared_hook(void (*)(void));

#endif
