#include "whitelist.h"

#include "handle.h"

#include <dlfcn.h>
#include <string.h>

static const char *WHITELIST[] = {
	// [Runtime] dynamic linker:
	"/ld-linux-x86-64.so.",
	"/libdl.so.",

	// Standard OS/language facilities:
	"/libc.so.",
	"/libpthread.so.",
	"/libstdc++.so.",

	// Debugging facilities:
	//
	// As of writing, the rr runtime ships with a PT_GNU_RELRO program header entry that
	// describes a segment to be made read only at load time that includes the GOT.  While this
	// wouldn't ordinarily be a problem because the shared library is also tagged as BIND_NOW
	// via both DT_FLAGS and DT_FLAGS_1 dynamic section entries to prevent lazy dynamic symbol
	// resolution, it means we unexpectedly fail to install the trampoline entries into the GOT.
	//
	// Some other options for actually detecting libraries with this setup:
	//  * Check whether .got.plt is aligned to a page and, if so, assume it might be protected.
	//  * Map the program header in from disk and check for an overlapping PT_GNU_RELRO entry.
	//  * Temporarily catch segfaults and use them to conclude that the page is protected.
	"/librrpreload.so",
};

struct whitelist;

void whitelist_shared_insert(struct whitelist *, const char *);

void whitelist_shared_init(struct whitelist *out) {
	for(const struct link_map *l = (struct link_map *) dlopen(NULL, RTLD_LAZY); l; l = l->l_next)
		if(whitelist_so_contains(l->l_name)) {
			const struct handle *h = handle_get(l, handle_init, NULL);
			for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st)
				if(st->st_shndx != SHN_UNDEF && st->st_shndx != SHN_ABS)
					whitelist_shared_insert(out, h->strtab + st->st_name);
			break;
		}
}

bool whitelist_so_contains(const char *path) {
	for(const char **it = WHITELIST;
		it != WHITELIST + sizeof WHITELIST / sizeof *WHITELIST;
		++it)
		if(strstr(path, *it))
			return true;
	return false;
}
