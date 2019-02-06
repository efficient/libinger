#include "ctestfuns.h"

#include <cassert>
#include <dlfcn.h>

using std::function;

static void fun(void) {}

void (*make_func(void))(void) {
	return fun;
}

void (*make_fnc(void))(void) {
	return [] {};
}

function<void(void)> make_fn(void) {
	static bool toggle;
	return [&] {
		toggle = !toggle;
	};
}

bool *mirror_mirror(void) {
	static bool samplib;
	return &samplib;
}

extern "C" {
void sync(void);
}

void sync(void) {
	static bool anchor;
	Dl_info dli;
	void *l;
	assert(dladdr1(&anchor, &dli, &l, RTLD_DL_LINKMAP));

	Lmid_t n;
	assert(!dlinfo(l, RTLD_DI_LMID, &n));
	assert(!n);
}
