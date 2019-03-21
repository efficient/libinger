#ifndef HANDLE_H_
#define HANDLE_H_

#include "error.h"
#include "namespace.h"

#include <sys/types.h>
#include <link.h>

struct link_map;
struct plot;
struct sym_hash;

struct shadow_gots {
	size_t override_table;
	unsigned first_entry;

	// When present, pointers into a single owned buffer.
	uintptr_t *gots[NUM_SHADOW_NAMESPACES + 1];

	// Owned.
	const struct plot *plot;
};

struct handle {
	const char *path;
	uintptr_t baseaddr;
	bool owned;
	bool eager;
	struct shadow_gots *shadow; // Not always present, but owned when it is.

	const ElfW(Sym) *symtab;
	const ElfW(Sym) *symtab_end;
	const char *strtab;

	const ElfW(Rela) *jmpslots; // Not always present.
	const ElfW(Rela) *jmpslots_end;
	const ElfW(Rela) *miscrels; // Not always present.
	const ElfW(Rela) *miscrels_end;

	const ElfW(Phdr) *jmpslots_seg; // Only present if jmpslots is unwritable.
	const ElfW(Phdr) *lazygot_seg; // Only present if lazy GOT is unwritable.
	const ElfW(Phdr) *eagergot_seg; // Only present if eager GOT is unwritable.

	size_t ntramps;
	size_t ntramps_symtab;
	size_t *tramps; // Owned.
};

enum error handle_init(struct handle *, const struct link_map *, struct link_map *);
void handle_cleanup(struct handle *);

// Set the function pointer to NULL to check for an existing handle, or to an initialization
// function to create a new handle if one doesn't already exist.  The error code pointer is only
// updated if it was non-NULL.
const struct handle *handle_get(
	const struct link_map *,
	enum error (*)(struct handle *, const struct link_map *),
	enum error *);

enum error handle_update(const struct link_map *, enum error (*)(struct handle *));

const ElfW(Sym) *handle_symbol(const struct handle *, const char *);

enum error handle_got_shadow(struct handle *);

static inline size_t handle_got_num_entries(const struct handle *h) {
	return h->ntramps;
}

#endif
