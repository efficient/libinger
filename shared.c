#include "shared.h"

#include "namespace.h"

#include <signal.h>
#include <stddef.h>

bool shared_pretrampoline = false;
void (*shared_trampoline)(void) = NULL;

void shared_hook(void (*hook)(void)) {
	shared_trampoline = hook;
}

void shared_prehook(void (*hook)(void)) {
	struct sigaction sa = {
		.sa_handler = (void (*)(int)) hook,
	};
	sigaction(SIGTRAP, &sa, NULL);
	shared_pretrampoline = true;
}
