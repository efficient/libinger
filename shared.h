#ifndef SHARED_H_
#define SHARED_H_

#include <stdbool.h>

// Check whether the current thread is currently executing shared code.  One important implication
// of this is that it is not async-cancel-safe.
bool shared_thread(void);

// The provided callback is invoked once whenever switching from shared code to a private copy.  The
// provided function must not perform any floating-point calculations!
void shared_hook(void (*hook)(void));

#endif
