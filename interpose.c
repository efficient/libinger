#include "interpose.h"

#include "segprot.h"

#include <sys/mman.h>
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

static const ElfW(Sym) *sym(const char *name, const ElfW(Sym) *symtab, const char *strtab) {
	const ElfW(Sym) *symtabe = (ElfW(Sym) *) strtab;
	for(const ElfW(Sym) *st = symtab; st != symtabe; ++st)
		if(st->st_shndx != SHN_UNDEF && !strcmp(strtab + st->st_name, name))
			return st;
	return NULL;
}

static inline void rela(const ElfW(Rela) *r, uintptr_t addr, const ElfW(Sym) *st, const char *s,
	void *(*dlsym)(void *, const char *)) {
	st += ELF64_R_SYM(r->r_info);
	if(st->st_shndx != SHN_UNDEF && ELF64_ST_TYPE(r->r_info) != STT_OBJECT) {
		const void *imp = dlsym(RTLD_NEXT, s += st->st_name);
		if(imp)
			*(const void **) (addr + r->r_offset) = imp;
	}
}

static void *dlsymb(void *handle, const char *symbol) {
	void *_dl_sym(void *, const char *, void *(*)(void *, const char *));
	return _dl_sym(handle, symbol, dlsymb);
}

void interpose_init(void) {
	const ElfW(Rela) *rel = dyn(DT_RELA);
	const ElfW(Rela) *rele = (ElfW(Rela) *) ((uintptr_t) rel + (size_t) dyn(DT_RELASZ));
	const ElfW(Rela) *jmprel = dyn(DT_JMPREL);
	const ElfW(Rela) *jmprele = (ElfW(Rela) *) ((uintptr_t) jmprel + (size_t) dyn(DT_PLTRELSZ));
	const ElfW(Sym) *symtab = dyn(DT_SYMTAB);
	const char *strtab = dyn(DT_STRTAB);

	uintptr_t addr;
	void *(*dls)(void *, const char *) = sym("dlsym", symtab, strtab) ? dlsymb : dlsym;
	const ElfW(Sym) *dlo = sym("dlopen", symtab, strtab);
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
			rela(r, addr, symtab, strtab, dls);
	prot_segment(addr, relseg, 0);

	prot_segment(addr, jmprelseg, PROT_WRITE);
	for(const ElfW(Rela) *r = jmprel; r != jmprele; ++r)
		rela(r, addr, symtab, strtab, dls);
	prot_segment(addr, jmprelseg, 0);
}
