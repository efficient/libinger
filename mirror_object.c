#include "mirror_object.h"

#include <assert.h>
#include <limits.h>
#include <link.h>
#include <stdbool.h>
#include <stdlib.h>
#include <string.h>

// Returns NULL on error.
static const char *progname(void) {
	extern const char *__progname_full;
	static char progname[PATH_MAX];
	static bool ready;

	const char *res = progname;
	// This can race during initialization, but it should still be correct because realpath()
	// will always populate progname with the exact same contents.
	if(!ready && (res = realpath(__progname_full, progname)))
		ready = true;
	return res;
}

enum error mirror_object(const struct link_map *l, const char *fname) {
	assert(l);
	assert(fname);

	if(l->l_name && *l->l_name) {
		if(fname && strcmp(fname, l->l_name))
			return ERROR_FNAME_MISMATCH;
	} else if(!fname && !(fname = progname()))
		return ERROR_FNAME_REALPATH;

	return SUCCESS;
}
