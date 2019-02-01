#include "mirror_object_containing.h"

#include <sys/mman.h>
#include <assert.h>
#include <link.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static void (**got)(void);
static void (*nope)(void);

static void (**unmemoize(void (**mem)(void), const char *sym))(void) {
	const struct link_map *l = dlopen(NULL, RTLD_LAZY);
	if(!l)
		return NULL;
	const ElfW(Dyn) *d;

	for(d = l->l_ld; d->d_tag != DT_STRTAB; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const char *str = (char *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_SYMTAB; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Sym) *st = (ElfW(Sym) *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_JMPREL; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Rela) *rel = (ElfW(Rela) *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_PLTRELSZ; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Rela) *end = (ElfW(Rela) *) ((uintptr_t) rel + d->d_un.d_val);

	const ElfW(Rela) *r;
	for(r = rel; strcmp(sym, str + st[ELF64_R_SYM(r->r_info)].st_name); ++r)
		if(r + 1 == end)
			return NULL;

	// Return the address of the GOT entry containing the address of the PLT trampoline.
	void (**res)(void) = (void (**)(void)) (l->l_addr + r->r_offset);

	size_t pgsz = sysconf(_SC_PAGESIZE);
	void *page = (void *) ((uintptr_t) &r->r_offset & ~(pgsz - 1));
	if(mprotect(page, pgsz, PROT_READ | PROT_WRITE))
		return NULL;

	// Trick future calls to the PLT trampoline into updating nope in lieu of the GOT entry.
	((ElfW(Rela) *) r)->r_offset = (uintptr_t) mem - l->l_addr;

	if(mprotect(page, pgsz, PROT_READ))
		return NULL;

	return res;
}

static void __attribute__((constructor)) ctor(void) {
	assert(got = unmemoize(&nope, "nop"));
}

bool mirror(const void *function) {
	assert(unmemoize(got, "nop"));
	enum error res = mirror_object_containing(function);
	assert(unmemoize(&nope, "nop"));
	return !res;
}

void nop(void) {}

void (*nop_location(void))(void) {
	if(nope != nop) {
		(*got)();
		assert(nope);
	}
	return nope;
}
