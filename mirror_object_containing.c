#include "mirror_object_containing.h"

#include "mirror_object.h"

#include <assert.h>
#include <dlfcn.h>
#include <stdlib.h>

enum error mirror_object_containing(const void *function) {
	return test_object_containing(mirror_object, function);
}

enum error test_object_containing(
	enum error (*plugin)(const struct link_map *, const char *),
	const void *function
) {
	Dl_info dli;
	struct link_map *l = NULL;
	if(!dladdr1(function, &dli, (void *) &l, RTLD_DL_LINKMAP))
		return ERROR_DLADDR;

	const char *fname = dli.dli_fname;
	if(!fname || !*fname)
		return ERROR_DLI_FNAME;

	return plugin(l, fname);
}
