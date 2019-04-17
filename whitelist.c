#include "whitelist.h"

#include "handle.h"
#include "namespace.h"

#include <assert.h>
#include <dlfcn.h>
#include <string.h>

struct whitelist;

static const char *WHITELIST[] = {
	// [Runtime] dynamic linker:
	// Although the dynmaic linker internally enforces that there is only a single instance of
	// itself, we need to whitelist it so our trampolines are aware of the namespace switch;
	// otherwise, our namespace accounting could become incorrect upon calls into it (in which
	// case we would also fail to invoke any client-provided hook function on the way back out).
	"/ld-linux-x86-64.so.",
	"/libdl.so.",

	// Standard OS/language facilities:
	// The primary issue here is the dynamic allocator: we can't have multiple versions hanging
	// around with different free lists!
	"/libc.so.",

	// POSIX threading:
	// According to https://sourceware.org/glibc/wiki/LinkerNamespaces, calling into multiple
	// copies of this library can cause observable state inconsistencies between the threads of
	// a single process.
	"/libpthread.so.",
};

void whitelist_shared_insert(struct whitelist *, const char *, uintptr_t);

void whitelist_so_insert_with(const struct handle *h, struct whitelist *out, bool me) {
	assert(h);
	assert(out);

	for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st)
		if(st->st_shndx != SHN_UNDEF)
			whitelist_shared_insert(out, h->strtab + st->st_name,
				me ? h->baseaddr + st->st_value : 0);
}

bool whitelist_so_contains(const char *path) {
	for(const char **it = WHITELIST;
		it != WHITELIST + sizeof WHITELIST / sizeof *WHITELIST;
		++it)
		if(strstr(path, *it))
			return true;
	return false;
}

void whitelist_shared_init(struct whitelist *out) {
	for(const struct link_map *l = (struct link_map *) dlopen(NULL, RTLD_LAZY); l; l = l->l_next) {
		bool myself = l == namespace_self();
		if(whitelist_so_contains(l->l_name) || myself)
			whitelist_so_insert_with(handle_get(l, NULL, NULL), out, myself);
	}
}
