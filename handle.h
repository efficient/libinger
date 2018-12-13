#ifndef HANDLE_H_
#define HANDLE_H_

#include "error.h"

#include <sys/types.h>
#include <link.h>
#include <stdbool.h>

#define GOT_GAP -3

struct link_map;
struct sym_hash;

struct got {
	uint64_t reserved;
	const struct link_map *l;
	void (*f)(void);
	const void *e[];
};

struct handle {
	const char *path;
	struct got *got;
	ssize_t got_start;
	const ElfW(Rela) *pltrel; // Not always present.
	const ElfW(Rela) *pltrel_end; // Not always present.
	const ElfW(Rela) *miscrel;
	const ElfW(Rela) *miscrel_end;
	const ElfW(Sym) *symtab;
	const ElfW(Sym) *symtab_end;
	const struct sym_hash *symhash; // Not always present.
	const char *strtab;
};

enum error handle_init(struct handle *, const struct link_map *);
const ElfW(Sym) *handle_symbol(const struct handle *, const char *);

#endif
