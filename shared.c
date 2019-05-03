#include "shared.h"

#include "namespace.h"

static void nop(void) {}

void (*shared_trampoline)(void) = nop;

void shared_hook(void (*hook)(void)) {
	shared_trampoline = hook ? hook : nop;
}
