#include <assert.h>
#include <link.h>
#include <stdbool.h>
#include <stdio.h>
#include <string.h>

extern const bool global_rodata;
extern bool global_data;
extern bool global_bss;

void code_direct(void);
void code_indirect(void);

static inline const char *objname(void) {
	const struct link_map *l;
	for(l = dlopen(NULL, RTLD_LAZY); l->l_ld != _DYNAMIC; l = l->l_next)
		assert(l);

	extern const char *const __progname;
	return *l->l_name ? strrchr(l->l_name, '/') + 1 : __progname;
}

int main(void) {
	printf("%s:main()\n", objname());

	code_direct();

	void (*volatile addr)(void);
	(addr = code_indirect)();

	return global_rodata ^ global_data ^ global_bss;
}
