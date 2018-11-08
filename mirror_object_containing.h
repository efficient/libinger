#ifndef MIRROR_OBJECT_CONTAINING_
#define MIRROR_OBJECT_CONTAINING_

enum error;
struct link_map;

enum error mirror_object_containing(const void *function);

enum error test_object_containing(
	enum error (*plugin)(const struct link_map *object, const char *path),
	const void *function);

#endif
