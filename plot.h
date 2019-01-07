#ifndef PLOT_H_
#define PLOT_H_

// Set this just small enough that the code doesn't outgrow a page.
#define PLOT_ENTRIES_PER_PAGE 406

#ifndef _asm
#include <stddef.h>
#include <stdint.h>

extern struct goot *const plot_template[];
extern const size_t plot_size;
extern const size_t plot_entries_offset;
extern const size_t plot_entry_size;
#endif

#endif
