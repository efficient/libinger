BINDGEN := bindgen
RUSTC := rustc

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override CXXFLAGS := -std=c++11 -O2 -Wall -Wextra -Wpedantic $(CXXFLAGS)
override RUSTFLAGS := --edition 2018 -Copt-level=2 $(RUSTFLAGS)

REVISION := HEAD

DEPS := libmirror_object.a goot.rs handle.rs handle_storage.rs mirror.rs plot_storage.rs whitelist_copy.rs whitelist_shared.rs
ELFLIBS := -lasm -lebl_x86_64
ELFUTILS := /usr/lib/x86_64-linux-gnu/elfutils
LINKRPATH := -L$(ELFUTILS) -Wl,-R$(ELFUTILS)

libgotcha.rlib: private LDFLAGS += -L.
libgotcha.rlib: private LDLIBS += -lmirror_object
libgotcha.rlib: $(DEPS)

libgotcha.a: $(DEPS)

libgotcha.so: private LDFLAGS += -L. $(LINKRPATH)
libgotcha.so: private LDLIBS += -lmirror_object $(ELFLIBS)
libgotcha.so: $(DEPS)

libgotchapreload.so: private LDFLAGS += -Wl,--exclude-libs,ALL $(LINKRPATH)
libgotchapreload.so: private LDLIBS += -ldl -lpthread $(ELFLIBS)
libgotchapreload.so: libgotcha.a

ctests: private CXXFLAGS += -Wno-pedantic -Wno-cast-function-type
ctests: private LDFLAGS += -Wl,-R\$$ORIGIN $(LINKRPATH)
ctests: private LDLIBS += -ldl -lpthread $(ELFLIBS)
ctests: libgotcha.a libctestfuns.so

bench: private LDFLAGS += -L. -Wl,-R\$$ORIGIN -Wl,-zlazy $(LINKPATH)
bench: private LDLIBS += -lbenchmark -lgotcha $(ELFLIBS)
bench: private RUSTFLAGS += --test
bench: libgotcha.a libbenchmark.so

libmirror_object.a: error.o globals.o goot.o handle.o interpose.o mirror_object_containing.o namespace.o plot.o segprot.o shared.o whitelist.o

libctestfuns.so: private CC := c++

goot.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
goot.rs: plot.h
handle.rs: private BINDFLAGS += --no-rustfmt-bindings --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
handle.rs: private CPPFLAGS += -D_GNU_SOURCE
handle.rs: error.h namespace.h
mirror.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types)]"
mirror.rs: mirror_object.h mirror_object_containing.h error.h

benchmark.o: private CFLAGS += -fpic
benchmark.o: private CPPFLAGS += -D_GNU_SOURCE -UNDEBUG
ctestfuns.o: ctestfuns.h
error.o: private CPPFLAGS += -isystem .
error.o: error.h
globals.o: private CFLAGS += -fpic
globals.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
globals.o: globals.h error.h goot.h handle.h namespace.h plot.h
goot.o: private CFLAGS += -fpic
goot.o: private CPPFLAGS += -D_GNU_SOURCE
goot.o: goot.h handle.h plot.h
gotchapreload.o: private CFLAGS += -fpic
gotchapreload.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: private CFLAGS += -fpic -Wno-array-bounds
handle.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: handle.h error.h goot.h namespace.h plot.h segprot.h
interpose.o: private CPPFLAGS += -D_GNU_SOURCE
interpose.o: interpose.h segprot.h
mirror_object.o: private CFLAGS += -fpic
mirror_object.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object.o: mirror_object.h error.h globals.h handle.h namespace.h threads.h whitelist.h
mirror_object_containing.o: private CPPFLAGS += -D_GNU_SOURCE
mirror_object_containing.o: mirror_object_containing.h mirror_object.h error.h
namespace.o: private CFLAGS += -fpic -ftls-model=initial-exec
namespace.o: private CPPFLAGS += -D_GNU_SOURCE
namespace.o: namespace.h threads.h
plot.o: private CPPFLAGS += -D_asm
plot.o: plot.h handle.h
segprot.o: segprot.h plot.h
shared.o: private CFLAGS += -fpic
shared.o: private CPPFLAGS += -D_GNU_SOURCE
shared.o: shared.h namespace.h
whitelist.o: private CPPFLAGS += -D_GNU_SOURCE
whitelist.o: whitelist.h handle.h namespace.h

libgotcha.tar:
	git archive --prefix=libgotcha/ -o $@ $(REVISION)
	mkdir -p libgotcha/.git/objects libgotcha/.git/refs
	echo "ref: refs/" >libgotcha/.git/HEAD
	git log --oneline --decorate=short --abbrev-commit -1 $(REVISION) >libgotcha/VERSION
	tar rf $@ libgotcha

.PHONY: clean
clean:
	git clean -ffdX

%: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) $< $(LDLIBS)
	@if objdump -p $@ | grep '\<TEXTREL\>' >/dev/null; then echo "WARNING: Generated object contains text relocations"; fi

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
	@if objdump -p $@ | grep '\<TEXTREL\>' >/dev/null; then echo "WARNING: Generated object contains text relocations"; fi

lib%.so: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic $< $(LDLIBS)
	@if objdump -p $@ | grep '\<TEXTREL\>' >/dev/null; then echo "WARNING: Generated object contains text relocations"; fi
