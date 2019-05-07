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

static bool libc(const char *sym) {
	if(config_sharedlibc())
		return true;

	return strcmp(sym, "fopen") && strcmp(sym, "fdopen") && strcmp(sym, "fclose") &&
		strcmp(sym, "fmemopen") && strcmp(sym, "open_memstream") &&
		!strstr(sym, "freopen") && !strstr(sym, "fflush") &&
		!strstr(sym, "setbuf") && strcmp(sym, "setlinebuf") && strcmp(sym, "setvbuf") &&
		!strstr(sym, "putc") && !strstr(sym, "puts") && !strstr(sym, "printf") &&
		!strstr(sym, "getc") && !strstr(sym, "gets") && !strstr(sym, "scanf") &&
		strcmp(sym, "getline") && strcmp(sym, "getdelim") &&
		strcmp(sym, "putw") && strcmp(sym, "getw") &&
		!strstr(sym, "fread") && !strstr(sym, "fwrite") &&
		!strstr(sym, "fseek") && !strstr(sym, "ftell") && strcmp(sym, "rewind") &&
		!strstr(sym, "fgetpos") && !strstr(sym, "fsetpos") &&
		!strstr(sym, "clearerr") && !strstr(sym, "feof") && !strstr(sym, "ferror") &&
		!strstr(sym, "stdin") && !strstr(sym, "stdout") && !strstr(sym, "stderr") &&
		!strstr(sym, "fileno");
}

static const struct patterns WHITELIST[] = {
	// [Runtime] dynamic linker:
	// Although the dynmaic linker internally enforces that there is only a single instance of
	// itself, we need to whitelist it so our trampolines are aware of the namespace switch;
	// otherwise, our namespace accounting could become incorrect upon calls into it (in which
	// case we would also fail to invoke any client-provided hook function on the way back out).
	{"/ld-linux-x86-64.so.", yes},
	{"/libdl.so.", yes},

	// Standard OS/language facilities:
	// The primary issue here is the dynamic allocator: we can't have multiple versions hanging
	// around with different free lists!
	{"/libc.so.", libc},

	// POSIX threading:
	// According to https://sourceware.org/glibc/wiki/LinkerNamespaces, calling into multiple
	// copies of this library can cause observable state inconsistencies between the threads of
	// a single process.
	{"/libpthread.so.", yes},
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
