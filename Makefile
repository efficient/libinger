BINDGEN := bindgen
OBJCOPY := objcopy
RUSTC := rustc

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override CXXFLAGS := -std=c++11 -O2 -Wall -Wextra -Wpedantic $(CXXFLAGS)
override LDFLAGS := $(LDFLAGS)
override LDLIBS := $(LDLIBS)
override RUSTFLAGS := --edition 2018 -Copt-level=2 $(RUSTFLAGS)

REVISION := HEAD

ELFUTILS := /usr/lib/x86_64-linux-gnu/elfutils

libgotchapreload.so: private LDFLAGS += -L$(ELFUTILS) -Wl,-R$(ELFUTILS) -zinitfirst -znodelete -znoexecstack
libgotchapreload.so: private LDLIBS += $(wildcard /usr/lib/x86_64-linux-gnu/libstd-*.so) -lasm -ldl -lebl_x86_64 -pthread
libgotchapreload.so: libgotcha.o

libgotcha.o: error.o globals.o goot.o handle.o interpose.o mirror.o namespace.o plot.o segprot.o shared.o whitelist.o
gotcha.o: goot.rs handle.rs handle_storage.rs mirror.rs plot_storage.rs whitelist_shared.rs

goot.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
goot.rs: plot.h
handle.rs: private BINDFLAGS += --no-rustfmt-bindings --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
handle.rs: private CPPFLAGS += -D_GNU_SOURCE
handle.rs: error.h namespace.h
mirror.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types)]"
mirror.rs: error.h

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
mirror.o: private CFLAGS += -fpic
mirror.o: private CPPFLAGS += -D_GNU_SOURCE
mirror.o: mirror.h error.h globals.h handle.h namespace.h threads.h whitelist.h
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

%.o: lib%.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type lib --emit obj -o $@ $< $(LDLIBS)

%.rs: %.h
	$(BINDGEN) $(BINDFLAGS) -o $@ $< -- $(CPPFLAGS)

lib%.a: %.o
	$(AR) rs $@ $^

lib%.a: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type staticlib $< $(LDLIBS)
	if [ -e $*.mri ]; then $(AR) -M <$*.mri; fi

lib%.o: %.o
	$(LD) $(LDFLAGS) -r -o $@ $^ $(LDLIBS)
	if [ -e $*.abi ]; then $(OBJCOPY) --keep-global-symbols=$*.abi $@; fi

lib%.rlib: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type rlib $< $(LDLIBS)

lib%.so: %.o
	$(CC) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)

lib%.so: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS) -zdefs -ztext" $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic $< $(LDLIBS)
