BINDGEN := bindgen
RUSTC := rustc

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override CXXFLAGS := -std=c++11 -O2 -Wall -Wextra -Wpedantic $(CXXFLAGS)
override RUSTFLAGS := -O $(RUSTFLAGS)

libgotcha.rlib: private RUSTFLAGS += -L.
libgotcha.rlib: private LDLIBS += -lmirror_object
libgotcha.rlib: libmirror_object.a mirror.rs whitelist_copy.rs

libgotcha.a: libmirror_object.a mirror.rs whitelist_copy.rs

ctests: private CXXFLAGS += -Wno-pedantic -Wno-cast-function-type
ctests: private LDFLAGS += -Wl,-R\$$ORIGIN
ctests: private LDLIBS += -ldl -lpthread
ctests: libgotcha.a libctestfuns.so

libmirror_object.a: error.o handle.o mirror_object_containing.o

mirror.rs: private BINDFLAGS += --raw-line "\#![allow(dead_code, non_camel_case_types)]"
mirror.rs: mirror_object.h mirror_object_containing.h error.h

ctestfuns.o: ctestfuns.h
error.o: error.h
handle.o: private CPPFLAGS += -D_DEFAULT_SOURCE
handle.o: handle.h error.h
mirror_object.o: mirror_object.h error.h
mirror_object_containing.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object_containing.o: mirror_object_containing.h mirror_object.h error.h

.PHONY: clean
clean:
	git clean -fX

%: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) $< $(LDLIBS)

%.rs: %.h
	$(BINDGEN) $(BINDFLAGS) -o $@ $< -- $(CPPFLAGS)

lib%.a: %.o
	$(AR) rs $@ $^

lib%.a: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type staticlib $< $(LDLIBS)
	if [ -e $*.mri ]; then ar -M <$*.mri; fi

lib%.rlib: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type rlib $< $(LDLIBS)

lib%.so: %.o
	$(CC) $(LDFLAGS) -shared -o $@ $^ $(LDLIBS)

lib%.so: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic $< $(LDLIBS)
