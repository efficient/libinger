#include <stdbool.h>

#ifdef global
#undef global
#define thread_local __thread
#else
#define thread_local static __thread
#endif

thread_local bool local_data = true;
thread_local bool local_bss;

void local_location(bool **data, bool **bss) {
	*data = &local_data;
	*bss = &local_bss;
}
