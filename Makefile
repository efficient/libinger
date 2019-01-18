BINDGEN := bindgen
RUSTC := rustc

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override CXXFLAGS := -std=c++11 -O2 -Wall -Wextra -Wpedantic $(CXXFLAGS)
override RUSTFLAGS := --edition 2018 -O $(RUSTFLAGS)

libgotcha.rlib: private RUSTFLAGS += -L.
libgotcha.rlib: private LDLIBS += -lmirror_object
libgotcha.rlib: libmirror_object.a goot.rs handle.rs handle_storage.rs mirror.rs plot_storage.rs whitelist_copy.rs whitelist_shared.rs

libgotcha.a: libmirror_object.a goot.rs handle.rs handle_storage.rs mirror.rs plot_storage.rs whitelist_copy.rs whitelist_shared.rs

ctests: private CXXFLAGS += -Wno-pedantic -Wno-cast-function-type
ctests: private LDFLAGS += -Wl,-R\$$ORIGIN
ctests: private LDLIBS += -ldl -lpthread
ctests: libgotcha.a libctestfuns.so

libmirror_object.a: error.o goot.o handle.o mirror_object_containing.o namespace.o plot.o whitelist.o

goot.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
goot.rs: plot.h
handle.rs: private BINDFLAGS += --no-rustfmt-bindings --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
handle.rs: private CPPFLAGS += -D_GNU_SOURCE
handle.rs: error.h namespace.h
mirror.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types)]"
mirror.rs: mirror_object.h mirror_object_containing.h error.h

ctestfuns.o: ctestfuns.h
error.o: error.h
goot.o: private CPPFLAGS += -D_GNU_SOURCE
goot.o: goot.h handle.h plot.h
handle.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: handle.h error.h namespace.h plot.h
mirror_object.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object.o: mirror_object.h error.h handle.h namespace.h whitelist.h
mirror_object_containing.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object_containing.o: mirror_object_containing.h mirror_object.h error.h
namespace.o: private CPPFLAGS += -D_GNU_SOURCE
namespace.o: namespace.h threads.h
plot.o: private CPPFLAGS += -D_asm
plot.o: plot.h handle.h
whitelist.o: private CPPFLAGS += -D_GNU_SOURCE
whitelist.o: whitelist.h handle.h namespace.h

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
