#include "mirror_object.h"

#include <assert.h>
#include <link.h>
#include <string.h>

enum error mirror_object(const struct link_map *l, const char *fname) {
	assert(l);
	assert(fname);
	if(l->l_name && *l->l_name && strcmp(l->l_name, fname))
		return ERROR_FNAME_MISMATCH;

	return SUCCESS;
}
