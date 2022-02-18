#ifndef HANDLE_H_
#define HANDLE_H_

#include "error.h"
#include "namespace.h"

#include <sys/types.h>
#include <link.h>

struct link_map;
struct plot;
struct sym_hash;

// NB: This structure is accessed from the main shadow trampoline, which is written in assembly; as
//     such, its structure should not be changed without also updating that code!
struct shadow_gots {
	size_t override_table;
	unsigned first_entry;
	size_t last_adjustment;

	// When present, pointers into a single owned buffer.
	uintptr_t *gots[NUM_SHADOW_NAMESPACES + 1];
};

struct restore {
	size_t size;
	uintptr_t off_loaded;
	void *addrs_stored[NUM_SHADOW_NAMESPACES];
};

struct handle {
	struct shadow_gots shadow; // Must be the first member so the trampoline can find it.

	char *path;
	uintptr_t baseaddr;
	bool vdso;
	bool owned;
	bool dependent;
	bool eager;
	bool sonamed;
	void (*ldaccess)(void);

	const ElfW(Phdr) *phdr;
	const ElfW(Phdr) *phdr_end;

	const ElfW(Sym) *symtab;
	const ElfW(Sym) *symtab_end;
	const char *strtab;

	const ElfW(Phdr) *tls; // Not always present.

	uintptr_t *globdats; // Only present if multiplexed between multiple namspaces.

	const ElfW(Rela) *jmpslots; // Not always present.
	const ElfW(Rela) *jmpslots_end;
	const ElfW(Rela) *miscrels; // Not always present.
	const ElfW(Rela) *miscrels_end;

	const ElfW(Phdr) *jmpslots_seg; // Only present if jmpslots is unwritable.
	const ElfW(Phdr) *lazygot_seg; // Only present if lazy GOT is unwritable.
	const ElfW(Phdr) *eagergot_seg; // Only present if eager GOT is unwritable.

	size_t ntramps;
	size_t ntramps_symtab;
	size_t *tramps; // Only present if ntramps is nonzero.  Owned.

	struct plot **plots; // Only present when tramps is.  Owned.

	size_t nrdwrs;
	struct restore *rdwrs; // Only present if multiplexed between multiple namespaces.
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

enum error handle_got_shadow(struct handle *);

// Whether it is safe to pass this to namespace_get() to retrieve info about a specific instance.
bool handle_is_get_safe(const struct handle *);

size_t handle_nodelete_pathlen(void);
bool handle_is_nodelete(const struct handle *);

static inline size_t handle_got_num_entries(const struct handle *h) {
	return h->ntramps;
}

bool handle_is_plot_storage_ready(void);

uintptr_t handle_symbol_plot(uintptr_t);

// Get the name of the executing program.  Returns NULL on error.
const char *handle_progname(void);

// Get the path to the interpreter (dynamic linker/loader).
const char *handle_interp_path(void);

#endif
