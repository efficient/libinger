#include "ctestfuns.h"

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
