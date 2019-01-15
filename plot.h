#ifndef PLOT_H_
#define PLOT_H_

// Set this just small enough that the code doesn't outgrow a page.
#define PLOT_ENTRIES_PER_PAGE 406

#ifndef _asm
#include <stddef.h>
#include <stdint.h>

struct handle;

struct plot {
	struct goot *goot;
	const uint8_t code[];
};

extern const struct plot plot_template;
extern const size_t plot_size;

// Relative to code member.
extern const size_t plot_entries_offset;
extern const size_t plot_entry_size;

// Idempotent.  Returns NULL if this library's GOT is too big to fit in any GOOT.
const struct plot *plot_insert_lib(struct handle *);

void plot_remove_lib(struct handle *);
#endif

#endif
