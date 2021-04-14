#define DYNAMIC_CONST
#include "dynamic.h"

#include "handle.h"
#include "segprot.h"

#include <assert.h>
#include <dlfcn.h>
#include <stddef.h>
#include <stdbool.h>
#include <string.h>

// These hooks will intercept all calls to the dynamic linker--internal implementation of the
// dlopen() and dlclose() functions, even those made by libc internally.  They must be dynamic
// symbol references so that they will induce a switch to the shared namespace; otherwise, any
// attempt to interrupt them could corrupt the dynamic linker state across *all* namespaces!
void libgotcha_dl_open(void);
void libgotcha_dl_close(void);

extern const uintptr_t _rtld_global_ro[];

static void *probe(void) {
	return NULL;
}

void *(*dynamic_open)(const char *, int, uintptr_t, Lmid_t, int, char **, char **);
void (*dynamic_close)(void *);

void dynamic_init(void) {
	const struct handle *ldso = handle_get(dlopen(handle_interp_path(), RTLD_LAZY), NULL, NULL);
	assert(ldso);
	assert(ldso->miscrels);

	const ElfW(Sym) *globro;
	for(globro = ldso->symtab; strcmp(ldso->strtab + globro->st_name, "_rtld_global_ro"); ++globro)
		assert(globro != ldso->symtab_end);

	const uintptr_t *_rtld_global_ro_end =
		(uintptr_t *) ((uintptr_t) _rtld_global_ro + globro->st_size);
	const ElfW(Phdr) *ro = segment_unwritable(
		(uintptr_t) _rtld_global_ro - ldso->baseaddr, ldso->phdr, ldso->phdr_end);
	prot_segment(ldso->baseaddr, ro, PROT_WRITE);

	const ElfW(Rela) *dlo;
	const void *notfound = dlopen(NULL, RTLD_LAZY);
	assert(notfound);
	for(dlo = ldso->miscrels; notfound; ++dlo) {
		assert(dlo != ldso->miscrels_end);

		uintptr_t *addr = (uintptr_t *) (ldso->baseaddr + dlo->r_offset);
		if(_rtld_global_ro <= addr && addr < _rtld_global_ro_end) {
			dynamic_open = (void *(*)(const char *, int, uintptr_t, Lmid_t, int, char **, char **)) *addr;
			*addr = (uintptr_t) probe;
			notfound = dlopen(NULL, RTLD_LAZY);
			*addr = (uintptr_t) dynamic_open;
		}
	}

	uintptr_t *dlp = (uintptr_t *) (ldso->baseaddr + dlo[-1].r_offset);
	*dlp = handle_symbol_plot((uintptr_t) libgotcha_dl_open);
	assert(*dlp);
	++dlp;
	dynamic_close = (void (*)(void *)) *dlp;
	*dlp = handle_symbol_plot((uintptr_t) libgotcha_dl_close);
	assert(*dlp);
	prot_segment(ldso->baseaddr, ro, 0);
}
