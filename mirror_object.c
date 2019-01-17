#include "mirror_object.h"

#include "handle.h"
#include "whitelist.h"

#include <assert.h>
#include <link.h>
#include <string.h>

static enum error hook_object(struct handle *h, const struct link_map *l) {
	// Do not create a handle for the vdso.
	if(l->l_name && *l->l_name && *l->l_name != '/')
		return SUCCESS;

	enum error code = handle_init(h, l);
	if(code)
		return code;

	if(whitelist_so_contains(h->path))
		return SUCCESS;

	return handle_got_shadow(h);
}

enum error mirror_object(const struct link_map *l, const char *fname) {
	assert(l);
	assert(fname);
	if(l->l_name && *l->l_name && strcmp(l->l_name, fname))
		return ERROR_FNAME_MISMATCH;

	whitelist_shared_contains(NULL);

	enum error fresh;
	for(fresh = -1; l && handle_get(l, hook_object, &fresh) && (signed) fresh <= SUCCESS; l = l->l_next);
	if((signed) fresh > SUCCESS)
		return fresh;

	return SUCCESS;
}
