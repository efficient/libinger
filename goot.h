#ifndef GOOT_H_
#define GOOT_H_

#include "plot.h"

#include <stdbool.h>

union goot_entry {
	const struct handle *lib;
	struct {
		unsigned odd_tag;
		unsigned next_free;
	} free;
};

struct goot {
	unsigned first_free;
	union goot_entry entries[PLOT_ENTRIES_PER_PAGE];
};

void goot_init(struct goot *table);

// Returns false if this table doesn't have enough remaining space.
bool goot_insert_lib(struct goot *table, const struct handle *object);

// Returns false if the specified entry is already free.
bool goot_remove_lib(struct goot *table, unsigned first_index);

#endif
