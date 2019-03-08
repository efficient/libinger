#include <stdbool.h>

extern const bool global_rodata;
extern bool global_data;
extern bool global_bss;

void code_direct(void);
void code_indirect(void);

int main(void) {
	code_direct();

	void (*volatile addr)(void);
	(addr = code_indirect)();

	return global_rodata ^ global_data ^ global_bss;
}
