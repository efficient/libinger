#include <sys/mman.h>
#include <assert.h>
#include <link.h>
#include <stdbool.h>
#include <stdint.h>
#include <string.h>
#include <unistd.h>

static void (*nope)(void);

static bool unmemoize(void (**mem)(void), const char *sym) {
	const struct link_map *l = dlopen(NULL, RTLD_LAZY);
	if(!l)
		return false;
	const ElfW(Dyn) *d;

	for(d = l->l_ld; d->d_tag != DT_STRTAB; ++d)
		if(d->d_tag == DT_NULL)
			return false;
	const char *str = (char *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_SYMTAB; ++d)
		if(d->d_tag == DT_NULL)
			return false;
	const ElfW(Sym) *st = (ElfW(Sym) *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_JMPREL; ++d)
		if(d->d_tag == DT_NULL)
			return false;
	const ElfW(Rela) *rel = (ElfW(Rela) *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_PLTRELSZ; ++d)
		if(d->d_tag == DT_NULL)
			return false;
	const ElfW(Rela) *end = (ElfW(Rela) *) ((uintptr_t) rel + d->d_un.d_val);

	const ElfW(Rela) *r;
	for(r = rel; strcmp(sym, str + st[ELF64_R_SYM(r->r_info)].st_name); ++r)
		if(r + 1 == end)
			return false;

	// Initialize nope to the address of the second instruction of the PLT trampoline function.
	nope = *(void (**)(void)) (l->l_addr + r->r_offset);

	size_t pgsz = sysconf(_SC_PAGESIZE);
	void *page = (void *) ((uintptr_t) &r->r_offset & ~(pgsz - 1));
	if(mprotect(page, pgsz, PROT_READ | PROT_WRITE))
		return false;

	// Trick future calls to the PLT trampoline into updating nope in lieu of the GOT entry.
	((ElfW(Rela) *) r)->r_offset = (uintptr_t) mem - l->l_addr;

	if(mprotect(page, pgsz, PROT_READ))
		return false;

	return true;
}

static void __attribute__((constructor)) ctor(void) {
	assert(unmemoize(&nope, "nop"));
}

void nop(void) {}

void (*nop_location(void))(void) {
	if(nope != nop)
		nope();
	return nope;
}
