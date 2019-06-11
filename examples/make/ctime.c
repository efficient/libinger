#include <stdio.h>
#include <time.h>

#pragma GCC push_options
#pragma GCC optimize("-fno-optimize-sibling-calls")
#pragma weak libctime_time = time
time_t time(time_t *tloc) {
	puts("time() from libctime");
	return time(tloc);
}
#pragma GCC pop_options
