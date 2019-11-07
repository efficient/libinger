#include "ancillary.h"

#include "plot.h"

#include <sys/auxv.h>
#include <sys/mman.h>
#include <link.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

static void nop(void) {}

static inline void purge_array(uint8_t **arr, size_t len) {
	const uint8_t *ret = (uint8_t *) (uintptr_t) nop;
	size_t pagesz = plot_pagesize();
	size_t mask = ~(pagesz - 1);
	for(size_t idx = 0; idx < len; ++idx) {
		void *page = (void *) ((uintptr_t) arr[idx] & mask);
		mprotect(page, pagesz, PROT_READ | PROT_WRITE | PROT_EXEC);
		*arr[idx] = *ret;
		mprotect(page, pagesz, PROT_READ | PROT_EXEC);
	}
}

bool ancillary_namespace(void) {
	#pragma weak _start
	void _start(void);

	void (*start)(void) = (void (*)(void)) getauxval(AT_ENTRY);
	if(start != _start) {
		const char *preload = getenv("LD_PRELOAD");
		if(!preload)
			return true;

		const struct link_map *l;
		for(l = dlopen(NULL, RTLD_LAZY); l && l->l_ld != _DYNAMIC; l = l->l_next);
		if(!l)
			return true;

		const char *name = strrchr(l->l_name, '/');
		return !strstr(preload, name ? name + 1 : l->l_name);
	}

	return false;
}

enum error ancillary_disable_ctors_dtors(void) {
	Dl_info dli;
	if(!dladdr((void *) (uintptr_t) ancillary_disable_ctors_dtors, &dli))
		return ERROR_DLADDR;

	uintptr_t addr = (uintptr_t) dli.dli_fbase;
	uint8_t **init = 0;
	size_t initlen = 0;
	uint8_t **fini = 0;
	size_t finilen = 0;
	for(ElfW(Dyn) *d = _DYNAMIC; d->d_tag != DT_NULL; ++d)
		switch(d->d_tag) {
		case DT_INIT_ARRAY:
			init = (uint8_t **) (addr + d->d_un.d_val);
			break;
		case DT_INIT_ARRAYSZ:
			initlen = d->d_un.d_val / sizeof *init;
			break;
		case DT_FINI_ARRAY:
			fini = (uint8_t **) (addr + d->d_un.d_val);
			break;
		case DT_FINI_ARRAYSZ:
			finilen = d->d_un.d_val / sizeof *fini;
			break;
		}

	if(init && initlen)
		purge_array(init, initlen);
	if(fini && finilen)
		purge_array(fini, finilen);

	return SUCCESS;
}
