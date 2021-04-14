#include "whitelist.h"

#include "config.h"
#include "handle.h"
#include "namespace.h"

#include <assert.h>
#include <dlfcn.h>
#include <string.h>

struct whitelist;

struct patterns {
	const char *pathname;
	bool (*symbol)(const char *);
};

static bool yes(const char *ign) {
	(void) ign;
	return true;
}

static bool silence(const char *ign) {
	(void) ign;
	return false;
}

static bool libc(const char *sym) {
	if(config_sharedlibc())
		return true;

	return strstr(sym, "malloc") || strstr(sym, "calloc") || strstr(sym, "realloc") ||
		strstr(sym, "valloc") || !strcmp(sym, "aligned_alloc") || strstr(sym, "memalign") ||
		!strcmp(sym, "free") || !strcmp(sym, "__libc_free") || !strcmp(sym, "__free_hook") ||
		!strcmp(sym, "cfree") || strstr(sym, "fork") || strstr(sym, "posix_spawn") ||
		strstr(sym, "uselocale") || !strcmp(sym, "__cxa_thread_atexit_impl");
}

static bool libpthread(const char *sym) {
	return !strcmp(sym, "pthread_create");
}

static const struct patterns WHITELIST[] = {
	// [Runtime] dynamic linker:
	// Although the dynmaic linker internally enforces that there is only a single instance of
	// itself, we need to whitelist it so our trampolines are aware of the namespace switch;
	// otherwise, our namespace accounting could become incorrect upon calls into it (in which
	// case we would also fail to invoke any client-provided hook function on the way back out).
	//
	// Note that whitelisting this does not capture the libdl case, since said library merely
	// calls into libc, which then uses an internal backdoor to call into ld.so.  The naive
	// approach would be to whitelist '/libdl.so.', but that is wrong for two reasons:
	//  * The functions implemented by libdl need to know which namespace they are called from.
	//    They determine this by checking their return address, but if we whitelist them, it
	//    will be set to our trampoline function, of which only the copy in the shared namespace
	//    ever runs.  Therefore, this approach would break ancillary namespaces' calls to
	//    dlopen(), dlsym(), and friends!
	//  * The glibc implementation leverages backdoor calls to ld.so in other contexts as well
	//    (e.g., the iconv interface, inet's IDNA facilities, and Name Service Switching).  Even
	//    if the above approach worked, we would have to whitelist the affected API service of
	//    each of these as well!
	// Instead, the 'dynamic' module hooks directly into the linker to handle such cases.
	{"/ld-linux-x86-64.so.", yes},

	// Don't actually whitelist this library for the above reasons, but silence the warning.
	{"/libdl.so.", silence},

	// Standard OS/language facilities:
	// The primary issue here is the dynamic allocator: we can't have multiple versions hanging
	// around with different free lists!
	{"/libc.so.", libc},

	// POSIX threading:
	// According to https://sourceware.org/glibc/wiki/LinkerNamespaces, calling into multiple
	// copies of this library can cause observable state inconsistencies between the threads of
	// a single process.
	{"/libpthread.so.", libpthread},
};

// Does not replace.
void whitelist_shared_insert(struct whitelist *, const char *, uintptr_t);

void whitelist_so_insert_with(const struct handle *h, struct whitelist *out,
	bool (*filter)(const char *), bool me) {
	assert(h);
	assert(out);
	assert(filter);

	for(const ElfW(Sym) *st = h->symtab; st != h->symtab_end; ++st) {
		const char *sym = h->strtab + st->st_name;
		if(st->st_shndx != SHN_UNDEF && filter(sym))
			whitelist_shared_insert(out, sym, me ? h->baseaddr + st->st_value : 0);
	}
}

bool whitelist_so_partial(const char *path) {
	bool (*res)(const char *) = whitelist_so_contains(path);
	return res && res != yes;
}

bool (*whitelist_so_contains(const char *path))(const char *) {
	for(const struct patterns *it = WHITELIST;
		it != WHITELIST + sizeof WHITELIST / sizeof *WHITELIST;
		++it)
		if(strstr(path, it->pathname))
			return it->symbol;
	return NULL;
}

void whitelist_shared_init(struct whitelist *out) {
	const struct link_map *self = namespace_self();
	whitelist_so_insert_with(handle_get(self, NULL, NULL), out, yes, true);
	for(const struct link_map *l = (struct link_map *) dlopen(NULL, RTLD_LAZY); l; l = l->l_next) {
		bool (*whitelisted)(const char *);
		if(l != self && (whitelisted = whitelist_so_contains(l->l_name)))
			whitelist_so_insert_with(handle_get(l, NULL, NULL), out, whitelisted, false);
	}
}
