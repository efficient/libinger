#include <stdio.h>

#pragma weak assert_static_repl
void assert_static_repl(void);

int main(int argc, char **argv) {
	(void) argc;

	if(!assert_static_repl) {
		printf("USAGE: LD_PRELOAD=make/libtls.so %s\n", *argv);
		return 1;
	}

	assert_static_repl();
	puts("Test PASSED: successfully called statically replaced function!");
	return 0;
}
