#include "error.h"
#include "ctestfuns.h"
extern "C" {
#include "goot.h"
#include "handle.h"
#include "mirror_object_containing.h"
#include "shared.h"
}

#include <cassert>
#include <climits>
#include <cstdlib>
#include <cstring>
#include <iostream>
#include <unistd.h>

using std::cerr;
using std::cout;
using std::endl;

static enum error contained_in_executable(const link_map *, const char *);
static enum error contained_in_library(const link_map *, const char *);
static void assert_success(enum error);

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

static void library_contains_func(void) {
	test_object_containing(contained_in_library, (void *) make_func);
	test_object_containing(contained_in_library, (void *) make_func());
}

static void library_contains_fnc(void) {
	test_object_containing(contained_in_library, (void *) make_fnc);
	test_object_containing(contained_in_library, (void *) make_fnc());
}

static void library_contains_fn(void) {
	test_object_containing(contained_in_library, (void *) make_fn);

	auto fun = make_fn();
	test_object_containing(contained_in_library, &fun);
}

static bool passed;

static void executable_handle_init(void) {
	struct handle h;
	struct link_map *l = (struct link_map *) dlopen(NULL, RTLD_LAZY);
	assert_success(handle_init(&h, l, l));
}

static void library_handle_init(void) {
	struct handle h;
	struct link_map *l = (struct link_map *) dlopen(NULL, RTLD_LAZY);
	while(!strstr(l->l_name, "/libctestfuns.so"))
		l = l->l_next;
	assert_success(handle_init(&h, l, l));
}

static void goot_abuse(void) {
	struct goot table;
	goot_init(&table);

	struct handle mock;
	memset(&mock, 0, sizeof mock);

	for(mock.ntramps = 1; mock.ntramps <= PLOT_ENTRIES_PER_PAGE + 1; ++mock.ntramps) {
		size_t entries;
		mock.shadow.override_table = mock.ntramps <= PLOT_ENTRIES_PER_PAGE ?
			table.identifier : 0;
		mock.shadow.first_entry = -1;
		for(entries = 0; goot_insert_lib(&table, &mock, 0); ++entries)
			mock.shadow.first_entry = -1;
		assert(entries == PLOT_ENTRIES_PER_PAGE / mock.ntramps);

		for(unsigned index = mock.shadow.first_entry = 0; index < PLOT_ENTRIES_PER_PAGE;
			mock.shadow.first_entry = ++index)
			assert(goot_remove_lib(&table, index) ==
				(!index || (!(index % mock.ntramps) &&
				PLOT_ENTRIES_PER_PAGE - index >= mock.ntramps)));
	}
}

static int invoked;

void callback(void) {
	++invoked;
}

static void mirror(void) {
	assert_success(mirror_object_containing((void *) fun));
	shared_hook(callback);

	assert(!*mirror_mirror());
	*mirror_mirror() = true;
	assert(*mirror_mirror());

	*namespace_thread() = 1;

	assert(!*mirror_mirror());
	*mirror_mirror() = true;
	assert(*mirror_mirror());

	assert(!invoked);
	sync();
	assert(*namespace_thread() == 1);
	assert(invoked == 1);

	assert(*mirror_mirror());
	*mirror_mirror() = false;
	assert(!*mirror_mirror());

	*namespace_thread() = 0;

	assert(*mirror_mirror());
	shared_hook(NULL);
}

static const struct {
	const char *const name;
	void (*const func)(void);
}TESTS[] = {
	{"executable_contains_func", executable_contains_func},
	{"executable_contains_fnc", executable_contains_fnc},
	{"executable_contains_fn", executable_contains_fn},
	{"library_contains_func", library_contains_func},
	{"library_contains_fnc", library_contains_fnc},
	{"library_contains_fn", library_contains_fn},
	{"executable_handle_init", executable_handle_init},
	{"library_handle_init", library_handle_init},
	{"goot_abuse", goot_abuse},
	{"mirror", mirror},
};

int main(int argc, const char **argv) {
	auto (*search)(const char *, const char *) = strstr;
	auto filter = (bool (*)(const char *, const char *)) search;
	int argb = 1;
	if(argc > 1 && *argv[argb] == '-') {
		if(strcmp(argv[argb], "--exact")) {
			printf("USAGE: %s [[--exact] <filter>...]\n", argv[0]);
			return 1;
		}
		filter = [](const char *left, const char *right) {
			return !strcmp(left, right);
		};
		++argb;
	}

	bool failed = false;
	for(auto test : TESTS) {
		bool found = argc == 1;
		for(int arg = argb; arg < argc; ++arg)
			if((found = filter(test.name, argv[arg])))
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

static enum error contained_in_library(const link_map *l, const char *fname) {
	(void) l;

	static char lname[PATH_MAX];
	if(!*lname) {
		auto succ = realpath("./libctestfuns.so", lname);
		assert(succ);
	}
	assert(fname);
	check_eq(lname, fname);
	return SUCCESS;
}

static void assert_success(enum error code) {
	if(code != SUCCESS)
		cerr << "warning: exited with error code " << code << endl;
	assert(code == SUCCESS);
}
