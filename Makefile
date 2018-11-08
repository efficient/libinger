BINDGEN := bindgen

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)

libmirror_object.a: error.o mirror_object_containing.o

mirror.rs: private BINDFLAGS += --raw-line "\#![allow(dead_code, non_camel_case_types)]"
mirror.rs: error.h mirror_object.h mirror_object_containing.h

error.o: error.h
mirror_object.o: private CPPFLAGS += -D_DEFAULT_SOURCE
mirror_object.o: mirror_object.h error.h
mirror_object_containing.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object_containing.o: mirror_object_containing.h error.h mirror_object.h

.PHONY: clean
clean:
	git clean -fX

%.rs: %.h
	$(BINDGEN) $(BINDFLAGS) -o $@ $< -- $(CPPFLAGS)

lib%.a: %.o
	$(AR) rs $@ $^

lib%.so: %.o
	$(CC) $(LDFLAGS) -shared -o $@ $^ $(LDLIBS)
