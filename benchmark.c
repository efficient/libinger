#include <sys/mman.h>
#include <assert.h>
#include <link.h>
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
#include <unistd.h>

#define nop _nop
#include "ancillary.c"
#undef nop

static inline const ElfW(Rela) *rel(const struct link_map *l, const char *s, bool lazy) {
	const ElfW(Dyn) *d;

	for(d = l->l_ld; d->d_tag != DT_STRTAB; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const char *str = (char *) d->d_un.d_ptr;

	for(d = l->l_ld; d->d_tag != DT_SYMTAB; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Sym) *st = (ElfW(Sym) *) d->d_un.d_ptr;

	ElfW(Sxword) reltag = lazy ? DT_JMPREL : DT_RELA;
	for(d = l->l_ld; d->d_tag != reltag; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Rela) *rel = (ElfW(Rela) *) d->d_un.d_ptr;

	ElfW(Sxword) endtag = lazy ? DT_PLTRELSZ : DT_RELASZ;
	for(d = l->l_ld; d->d_tag != endtag; ++d)
		if(d->d_tag == DT_NULL)
			return NULL;
	const ElfW(Rela) *end = (ElfW(Rela) *) ((uintptr_t) rel + d->d_un.d_val);

	const ElfW(Rela) *r;
	for(r = rel; strcmp(s, str + st[ELF64_R_SYM(r->r_info)].st_name); ++r)
		if(r + 1 == end)
			return NULL;
	return r;
}

static inline bool pageprot(const void *addr, int prot) {
	size_t pgsz = sysconf(_SC_PAGESIZE);
	void *page = (void *) ((uintptr_t) addr & ~(pgsz - 1));
	return !mprotect(page, pgsz, prot);
}

static inline bool unmemoize(void (**shadow)(void), const struct link_map *l, const ElfW(Rela) *r) {
	if(!pageprot(&r->r_offset, PROT_READ | PROT_WRITE))
		return false;

	// Trick future calls to the PLT trampoline into updating mem in lieu of the GOT entry.
	((ElfW(Rela) *) r)->r_offset = (uintptr_t) shadow - l->l_addr;

	if(!pageprot(&r->r_offset, PROT_READ))
		return false;

	return true;
}

static inline void (**addr(const struct link_map *l, const ElfW(Rela) *r))(void) {
	return (void (**)(void)) (l->l_addr + r->r_offset);
}

static inline void (**dataddr(const struct link_map *l, const char *sym))(void) {
	const ElfW(Rela) *r = rel(l, sym, false);
	return addr(l, r);
}

static void (**got)(void);
static void (**sgot)(void);
static void (*nope)(void);

static void __attribute__((constructor)) ctor(void) {
	if(ancillary_namespace())
		return;

	const struct link_map *l = dlopen(NULL, RTLD_LAZY);
	assert(l);

	void (*clock_gettime)(void);
	void (**clock)(void) = dataddr(l, "clock_gettime");
	assert(clock);
	clock_gettime = *clock;

	const ElfW(Rela) *r = rel(l, "nop", true);
	assert(r);

	// Save the address of the GOT entry containing the address of the PL(O)T trampoline.
	got = addr(l, r);

	// The loader initializes dependencies before LD_PRELOADs... unless the latter have
	// INITFIRST set... unless *any* of the former has INITFIRST set.  Fortunately, we're in the
	// latter case thanks to libpthread.  However, dlopen()'ing libgotcha here appears to
	// call its constructor immediately, even though it was already open.  We've already saved
	// the address of the (real) GOT entry, so let's go ahead and do that now!
	bool gotme = dlopen("libgotcha.so", RTLD_LAZY | RTLD_NOLOAD);

	// Save the shadow GOT entry, which will be the GOT entry itself unless libgotcha is loaded.
	sgot = addr(l, r);

	if(!unmemoize(&nope, l, r))
		abort();

	if(gotme) {
		assert(*clock != clock_gettime);

		// Roll back libgotcha's changes to clock_gettime() to exempt measurement from overheads.
		pageprot(clock, PROT_READ | PROT_WRITE);
		*clock = clock_gettime;
		pageprot(clock, PROT_READ);
	}
}

const bool no;

void nop(void) {}

static void (*nop_location(void))(void) {
	if(nope != nop) {
		(*got)();
		assert(nope);
	}
	return nope;
}

void with_eager_nop(void (*fun)(void)) {
	void (*plt)(void) = *sgot;
	*sgot = nop_location();
	fun();
	*sgot = plt;
}

size_t plot_pagesize(void) {
	return 0;
}

// Dummy symbols to allow linking without libgotcha.
void libgotcha_group_new(void) {}
void (*libgotcha_group_thread_accessor(void))(void) { return nop; }
void libgotcha_shared_hook(void) {}
void libgotcha_shared_prehook(void) {}
