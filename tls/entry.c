#include <dlfcn.h>
#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>

bool local_location(bool **, bool **);

int main(int argc, char **argv) {
	bool *data, *bss;
	local_location(&data, &bss);
	printf("l %2d d %#lx b %#lx\n", argc - 1, (uintptr_t) data, (uintptr_t) bss);

	void *l = dlmopen(LM_ID_NEWLM, argv[0], RTLD_LAZY);
	int (*m)(int, char **) = (int (*)(int, char **)) (uintptr_t) dlsym(l, "main");
	m(argc + 1, argv);

	return 0;
}
