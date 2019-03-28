#ifndef GOOT_H_
#define GOOT_H_

#include "plot.h"

#include <stdbool.h>

union goot_entry {
	struct handle *lib;
	struct {
		unsigned odd_tag;
		unsigned next_free;
	} free;
};

// NB: This structure is accessed from the main shadow trampoline, which is written in assembly; as
//     such, its structure should not be changed without also updating that code!
struct goot {
	size_t identifier;
	size_t adjustment;
	unsigned first_free;
	union goot_entry entries[PLOT_ENTRIES_PER_PAGE];
};

void goot_init(struct goot *table);

// Idempotent.  Returns false if this table doesn't have enough remaining space for the entire
// library; this may mean one of two possible things:
//  * If the table was nonempty before the operation, it is unmodified.
//  * If the table was empty before the operation, it has been completely populated with entries
//    corresponding to the appropriate range of indices.
bool goot_insert_lib(struct goot *table, struct handle *object, size_t tablenum);

// Returns false if the specified entry is already free.
bool goot_remove_lib(struct goot *table, unsigned first_index);

bool goot_empty(const struct goot *table);

#endif
