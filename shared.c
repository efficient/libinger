#include "shared.h"

#include "namespace.h"

#include <stddef.h>

void (*shared_trampoline)(void) = NULL;

void shared_hook(void (*hook)(void)) {
	shared_trampoline = hook;
}
