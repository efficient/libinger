#include <stdbool.h>
#include <stdio.h>

const bool global_rodata = true;
bool global_data = true;
bool global_bss;

void code_direct(void) {
	puts(__FILE__ ":code_direct()");
}

void code_indirect(void) {
	puts(__FILE__ ":code_indirect()");
}
