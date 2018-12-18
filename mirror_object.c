#include "mirror_object.h"

#include "handle.h"

#include <assert.h>
#include <link.h>
#include <string.h>

static enum error hook_object(struct handle *h, const struct link_map *l) {
	enum error code = handle_init(h, l);
	if(code)
		return code;

	return SUCCESS;
}

enum error mirror_object(const struct link_map *l, const char *fname) {
	assert(l);
	assert(fname);
	if(l->l_name && *l->l_name && strcmp(l->l_name, fname))
		return ERROR_FNAME_MISMATCH;

	enum error fresh;
	for(fresh = -1; l && handle_get(l, hook_object, &fresh) && fresh <= SUCCESS; l = l->l_next);
	if(fresh > SUCCESS)
		return fresh;

	return SUCCESS;
}
