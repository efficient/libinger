BINDGEN := bindgen
NM := nm
OBJCOPY := objcopy
RUSTC := rustc

override BINDFLAGS := --default-enum-style rust $(BINDFLAGS)
override CFLAGS := -std=c11 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override CXXFLAGS := -std=c++11 -O2 -Wall -Wextra -Wpedantic $(CXXFLAGS)
override LDFLAGS := $(LDFLAGS)
override LDLIBS := $(LDLIBS)
override RUSTFLAGS := --edition 2018 -Copt-level=2 $(RUSTFLAGS)

ELFUTILS := /usr/lib/x86_64-linux-gnu/elfutils
REVISION := HEAD

CGLOBALS := $(wildcard libgotcha_*.c)

.PHONY: all
all: libgotcha.a libgotcha.rlib libgotcha.so

libgotcha.a: libgotcha.o libgotcha_api.rs
libgotcha.rlib: libgotcha.o libgotcha_api.rs

libgotcha.so: private LDFLAGS += -L$(ELFUTILS) -Wl,-R$(ELFUTILS) -zinitfirst -znodelete -Wl,-zlazy
libgotcha.so: private LDFLAGS += libgotcha.o -lasm -lc -ldl -lebl_x86_64 -lpthread -lunwind
libgotcha.so: libgotcha.o libgotcha_api.rs

libgotcha.o: $(CGLOBALS:.c=.o) config.o error.o globals.o goot.o handle.o init.o interpose.o namespace.o plot.o segprot.o shared.o whitelist.o
gotcha.o: gotcha.abi goot.rs handle.rs handle_storage.rs plot_storage.rs whitelist_shared.rs

gotcha.abi: $(CGLOBALS:.c=.o)
	$(NM) -gP --defined-only $^ | grep -ve':$$' -e' \<W\>' | cut -d" " -f1 | sort >$@

bench: private LDFLAGS += -Wl,-zlazy -Wl,-R\$$ORIGIN
bench: private LDLIBS += -lbenchmark
bench: private RUSTFLAGS += --test -L.
bench: libbenchmark.so

libbenchmark.so: private LDLIBS += -ldl

goot.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
goot.rs: plot.h
handle.rs: private BINDFLAGS += --no-rustfmt-bindings --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
handle.rs: private CPPFLAGS += -D_GNU_SOURCE
handle.rs: error.h namespace.h
libgotcha_api.rs: private BINDFLAGS += --raw-line "\#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]"

benchmark.o: private CFLAGS += -fpic
benchmark.o: private CPPFLAGS += -D_GNU_SOURCE -UNDEBUG
config.o: private CFLAGS += -fpic
config.o: config.h
ctestfuns.o: ctestfuns.h
error.o: private CPPFLAGS += -isystem .
error.o: error.h
globals.o: private CFLAGS += -fpic
globals.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
globals.o: globals.h config.h error.h goot.h handle.h namespace.h plot.h threads.h
goot.o: private CFLAGS += -fpic
goot.o: private CPPFLAGS += -D_GNU_SOURCE
goot.o: goot.h handle.h plot.h
gotchapreload.o: private CFLAGS += -fpic
gotchapreload.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: private CFLAGS += -fpic -Wno-array-bounds
handle.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: handle.h config.h error.h goot.h namespace.h plot.h segprot.h
init.o: private CFLAGS += -fpic
init.o: private CPPFLAGS += -D_GNU_SOURCE
init.o: config.h globals.h handle.h interpose.h namespace.h whitelist.h
interpose.o: private CPPFLAGS += -D_GNU_SOURCE
interpose.o: interpose.h segprot.h
libgotcha_api.o: private CPPFLAGS += -D_GNU_SOURCE
libgotcha_api.o: libgotcha_api.h namespace.h shared.h
libgotcha_repl.o: private CFLAGS += -fno-optimize-sibling-calls -fpic
libgotcha_repl.o: private CPPFLAGS += -D_GNU_SOURCE
libgotcha_repl.o: config.h globals.h namespace.h
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

lib%.a: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type staticlib $< $(LDLIBS)
	if [ -e lib$*.o ]; then $(AR) rs $@ lib$*.o; fi

lib%.o: %.o
	$(LD) $(LDFLAGS) -r -o $@ $^ $(LDLIBS)
	if [ -e $*.abi ]; then $(OBJCOPY) --keep-global-symbols=$*.abi $@; fi

lib%.rlib: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type rlib $< $(LDLIBS)
	if [ -e lib$*.o ]; then $(AR) rs $@ lib$*.o; fi

lib%.so: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS) -zdefs -ztext" $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic $< $(LDLIBS)

lib%.so: %.o
	$(CC) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)
