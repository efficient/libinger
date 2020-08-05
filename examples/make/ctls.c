#include <stdint.h>

void *libgotcha_tls_get_addr(uintptr_t);

void assert_static_repl(void) {
	// Only libgotcha's static replacement can tolerate a null argument without crashing.
	libgotcha_tls_get_addr(0);
}
