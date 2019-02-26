#ifndef HANDLE_H_
#define HANDLE_H_

#define GOT_GAP -3

#ifndef _asm
#include "error.h"
#include "namespace.h"

#include <sys/types.h>
#include <link.h>

struct link_map;
struct sym_hash;

struct got {
	const uint64_t reserved;
	struct link_map *l;
	void (*f)(void);
	const void *e[];
};

struct shadow_gots {
	size_t override_table;
	unsigned first_entry;

	// Each of these entries points into a single owned buffer.  Within each ancillary GOT
	// (i.e., in each namespace with the exception of BASE) lies a strong (owned) reference to
	// a link_map.  Unlike the main link_map, each of these is owned jointly with any loaded
	// dependent libraries, and are therefore always dlclose()'d by handle_cleanup().
	struct got *gots[NUM_SHADOW_NAMESPACES + 1];
};

struct handle {
	const char *path;
	struct got *got;
	ssize_t got_start;
	ssize_t sgot_start;
	size_t got_len;
	struct shadow_gots *shadow; // Not always present, but owned when it is.
	const ElfW(Rela) *pltrel; // Not always present.
	const ElfW(Rela) *pltrel_end; // Not always present.
	const ElfW(Rela) *miscrel;
	const ElfW(Rela) *miscrel_end;
	int pltprot;
	int miscprot;
	const ElfW(Sym) *symtab;
	const ElfW(Sym) *symtab_end;
	const struct sym_hash *symhash; // Not always present.
	const char *strtab;
};

enum error handle_init(struct handle *, const struct link_map *);
void handle_cleanup(struct handle *);

// Set the function pointer to NULL to check for an existing handle, or to an initialization
// function to create a new handle if one doesn't already exist.  The error code pointer is only
// updated if it was non-NULL.
const struct handle *handle_get(
	const struct link_map *,
	enum error (*)(struct handle *, const struct link_map *),
	enum error *);

const ElfW(Sym) *handle_symbol(const struct handle *, const char *);

// Idempotent.
enum error handle_got_shadow(struct handle *);

static inline size_t handle_got_num_entries(const struct handle *h) {
	return -h->sgot_start + GOT_GAP + h->got_len;
}
#endif

#endif
