#include "error.h"
extern "C" {
#include "mirror_object_containing.h"
}

#include <cassert>
#include <climits>
#include <cstdlib>
#include <cstring>
#include <iostream>

using std::cerr;
using std::cout;
using std::endl;

static enum error contained_in_executable(const link_map *, const char *);

static void fun(void) {}

static void executable_contains_func(void) {
	assert(fun == &fun);
	test_object_containing(contained_in_executable, (void *) fun);
}

static void executable_contains_fnc(void) {
	void (*fun)(void) = [] {};
	test_object_containing(contained_in_executable, (void *) fun);
}

static void executable_contains_fn(void) {
	bool toggle;
	auto fun = [&] {
		toggle = !toggle;
	};
	test_object_containing(contained_in_executable, &fun);
}

static const struct {
	const char *const name;
	void (*const func)(void);
}TESTS[] = {
	{"executable_contains_func", executable_contains_func},
	{"executable_contains_fnc", executable_contains_fnc},
	{"executable_contains_fn", executable_contains_fn},
};

static bool passed;

int main(int argc, const char **argv) {
	bool failed = false;

	for(auto test : TESTS) {
		bool found = argc == 1;
		for(int arg = 1; arg < argc; ++arg)
			if((found = strstr(test.name, argv[arg])))
				break;
		if(!found)
			continue;

		cout << "test " << test.name << " ... ";
		passed = true;
		test.func();
		if(passed)
			cout << "ok" << endl;
		failed = failed || !passed;
	}

	return failed;
}

static bool check_eq(const char *left, const char *right) {
	if(!strcmp(right, left))
		return true;

	if(passed) {
		cout << "FAILED" << endl;
		cerr << "assertion failed: `(left == right)`" << endl
			<< "  left: `\"" << left << "\"`" << endl
			<< " right: `\"" << right << "\"`" << endl
		;
		passed = false;
	}
	return false;
}

static enum error contained_in_executable(const link_map *l, const char *fname) {
	(void) l;

	extern const char *__progname_full;
	assert(fname);
	check_eq(__progname_full, fname);
	return SUCCESS;
}
