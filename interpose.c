#include "interpose.h"

#include "handle.h"
#include "namespace.h"
#include "segprot.h"

#include <sys/mman.h>
#include <assert.h>
#include <link.h>
#include <stddef.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static inline const void *dyn(unsigned tag) {
	for(const ElfW(Dyn) *d = _DYNAMIC; d->d_tag != DT_NULL; ++d)
		if(d->d_tag == tag)
			return (void *) d->d_un.d_ptr;
	return NULL;
}

static const char *str(size_t offset) {
	static const char *tab;
	if(!tab)
		tab = dyn(DT_STRTAB);
	return tab + offset;
}

static const ElfW(Sym) *sym(const char *name, const ElfW(Sym) *symtab) {
	const ElfW(Sym) *symtabe = (ElfW(Sym) *) str(0);
	for(const ElfW(Sym) *st = symtab; st != symtabe; ++st)
		if(st->st_shndx != SHN_UNDEF && !strcmp(str(st->st_name), name))
			return st;
	return NULL;
}

static void rela(const ElfW(Rela) *r, uintptr_t addr, const ElfW(Sym) *st,
	void *(*dlsym)(void *, const char *)) {
	st += ELF64_R_SYM(r->r_info);
	if(st->st_shndx != SHN_UNDEF && ELF64_ST_TYPE(r->r_info) != STT_OBJECT) {
		const void *imp = dlsym(RTLD_NEXT, str(st->st_name));
		if(imp)
			*(const void **) (addr + r->r_offset) = imp;
	}
}

// Emulate a subset of dlsym()'s functionality.
// If our library defines a dlsym() interposition function, we can't use it during interposition
// bootstrapping for obvious reasons.  This function is a temporary replacement for this case.
static void *dlsymb(void *handle, const char *symbol) {
	void *__libc_dlsym(void *, const char *);

	// We do not support NULL/RTLD_DEFAULT!
	assert(handle);
	if(handle == RTLD_NEXT) {
		// Emulate RTLD_NEXT by iterating over our direct dependencies.  Any function we
		// call should be present in one of these modules!
		for(const ElfW(Dyn) *d = _DYNAMIC; d->d_tag != DT_NULL; ++d)
			if(d->d_tag == DT_NEEDED) {
				const char *s = str(d->d_un.d_val);
				void *l = s == handle_interp_path() ?
					dlopen(s, RTLD_LAZY) : namespace_get(0, s, RTLD_LAZY);
				void *res = __libc_dlsym(l, symbol);
				if(res)
					return res;
			}
	} else
		// The more limited __libc_dlsym() is fine for searching an individual module.
		return __libc_dlsym(handle, symbol);
	return NULL;
}

void interpose_init(void) {
	const ElfW(Rela) *rel = dyn(DT_RELA);
	const ElfW(Rela) *rele = (ElfW(Rela) *) ((uintptr_t) rel + (size_t) dyn(DT_RELASZ));
	const ElfW(Rela) *jmprel = dyn(DT_JMPREL);
	const ElfW(Rela) *jmprele = (ElfW(Rela) *) ((uintptr_t) jmprel + (size_t) dyn(DT_PLTRELSZ));
	const ElfW(Sym) *symtab = dyn(DT_SYMTAB);

	uintptr_t addr;
	void *(*dls)(void *, const char *) = sym("dlsym", symtab) ? dlsymb : dlsym;
	const ElfW(Sym) *dlo = sym("dlopen", symtab);
	if(dlo)
		addr = (uintptr_t) dlopen - dlo->st_value;
	else {
		const struct link_map *l;
		for(l = dlopen(NULL, RTLD_LAZY); l && l->l_ld != _DYNAMIC; l = l->l_next);
		addr = l->l_addr;
	}

	const ElfW(Ehdr) *e = (ElfW(Ehdr) *) addr;
	const ElfW(Phdr) *p = (ElfW(Phdr) *) (addr + e->e_phoff);
	const ElfW(Phdr) *pe = p + e->e_phnum;
	const ElfW(Phdr) *relseg = NULL;
	const ElfW(Phdr) *jmprelseg = NULL;
	if(rel != rele)
		relseg = segment_unwritable(rel->r_offset, p, pe);
	if(jmprel != jmprele)
		jmprelseg = segment_unwritable(jmprel->r_offset, p, pe);

	prot_segment(addr, relseg, PROT_WRITE);
	for(const ElfW(Rela) *r = rel; r != rele; ++r)
		if(ELF64_R_TYPE(r->r_info) == R_X86_64_GLOB_DAT)
			rela(r, addr, symtab, dls);
	prot_segment(addr, relseg, 0);

	prot_segment(addr, jmprelseg, PROT_WRITE);
	for(const ElfW(Rela) *r = jmprel; r != jmprele; ++r)
		rela(r, addr, symtab, dls);
	prot_segment(addr, jmprelseg, 0);
}
