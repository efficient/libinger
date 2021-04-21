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
all: libgotcha.a libgotcha.rlib libgotcha.so libgotcha.mk

libgotcha.a: libgotcha.o libgotcha_api.rs namespace.rs prctl.rs
libgotcha.rlib: libgotcha.o libgotcha_api.rs namespace.rs prctl.rs

libgotcha.so: private LDFLAGS += -L$(ELFUTILS) -Wl,-R$(ELFUTILS) -zinitfirst -Wl,-zlazy
libgotcha.so: private LDFLAGS += libgotcha.o -ldl -lpthread -lc -lasm -lebl_x86_64 -lunwind
libgotcha.so: libgotcha.o libgotcha_api.rs namespace.rs prctl.rs

libgotcha.mk: gotcha.mk libgotcha.so
	objdump -p $(@:.mk=.so) | sed -n 's/.*\<NEEDED\>.*lib\(std-.*\)\.so.*/ifndef LIBSTDRUST_SONAME\nLIBSTDRUST_SONAME := \1\nendif\n/p' | cat - $< >$@

libgotcha.o: $(CGLOBALS:.c=.o) ancillary.o config.o dynamic.o error.o globals.o goot.o handle.o handles.o init.o interpose.o namespace.o plot.o repl.o segprot.o shared.o tcb.o whitelist.o

gotcha.o: gotcha.abi goot.rs handle.rs handle_storage.rs plot_storage.rs whitelist_shared.rs
gotcha.o: private RUSTFLAGS += $(shell $(RUSTC) --version | ./cfg -)

gotcha.abi: $(CGLOBALS:.c=.o)
	$(NM) -gP --defined-only $^ | grep -ve':$$' -e' \<W\>' | cut -d" " -f1 | sort >$@

bench: private LDFLAGS += -Wl,-zlazy -Wl,-R\$$ORIGIN
bench: private LDLIBS += -lbenchmark
bench: private RUSTFLAGS += --test -L. --extern test=libtest.rlib
bench: libbenchmark.so libtest.rlib

libbenchmark.so: private LDLIBS += -ldl
libtest.rlib: private RUSTC := RUSTC_BOOTSTRAP= $(RUSTC)

goot.rs: private BINDFLAGS += --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]"
goot.rs: plot.h
handle.rs: private BINDFLAGS += --no-rustfmt-bindings --raw-line "\#![allow(non_camel_case_types, non_upper_case_globals)]" --blacklist-type La_x86_64_regs --blacklist-function la_x86_64_gnu_pltenter --blacklist-function la_x86_64_gnu_pltexit --blacklist-function la_x32_gnu_pltenter --blacklist-function la_x32_gnu_pltexit
handle.rs: private CPPFLAGS += -D_GNU_SOURCE
handle.rs: error.h namespace.h
libgotcha_api.rs: private BINDFLAGS += --raw-line "\#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]"
namespace.rs: private BINDFLAGS += --raw-line "\#![allow(dead_code, non_camel_case_types, non_upper_case_globals)]"
namespace.rs: private CPPFLAGS += -D_GNU_SOURCE

ancillary.o: private CPPFLAGS += -D_GNU_SOURCE
ancillary.o: ancillary.h error.h plot.h
benchmark.o: private CFLAGS += -fpic
benchmark.o: private CPPFLAGS += -D_GNU_SOURCE -UNDEBUG
benchmark.o: ancillary.c
config.o: private CFLAGS += -fpic
config.o: private CPPFLAGS += -D_GNU_SOURCE
config.o: config.h namespace.h
ctestfuns.o: ctestfuns.h
dynamic.o: private CFLAGS += -fpic
dynamic.o: private CPPFLAGS += -D_GNU_SOURCE
dynamic.o: handle.h segprot.h
error.o: private CPPFLAGS += -isystem .
error.o: error.h
globals.o: private CFLAGS += -fpic
globals.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
globals.o: globals.h config.h error.h goot.h handle.h namespace.h plot.h threads.h
goot.o: private CFLAGS += -fpic
goot.o: private CPPFLAGS += -D_GNU_SOURCE
goot.o: goot.h handle.h namespace.h plot.h
handle.o: private CFLAGS += -fpic -Wno-array-bounds
handle.o: private CPPFLAGS += -D_GNU_SOURCE
handle.o: handle.h config.h error.h goot.h namespace.h plot.h segprot.h
handles.o: private CFLAGS += -fpic
handles.o: private CPPFLAGS += -D_GNU_SOURCE
handles.o: handles.h config.h error.h handle.h namespace.h repl.h
init.o: private CFLAGS += -fpic
init.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
init.o: config.h error.h dynamic.h globals.h handle.h handles.h interpose.h namespace.h repl.h threads.h whitelist.h
interpose.o: private CPPFLAGS += -D_GNU_SOURCE
interpose.o: interpose.h handle.h namespace.h segprot.h
libgotcha_api.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
libgotcha_api.o: libgotcha_api.h config.h handle.h handles.h namespace.h repl.h shared.h
libgotcha_repl.o: private CFLAGS += -fno-optimize-sibling-calls -fpic
libgotcha_repl.o: private CPPFLAGS += -D_GNU_SOURCE -Wno-missing-attributes
libgotcha_repl.o: libgotcha_repl.h config.h dynamic.h globals.h handle.h handles.h namespace.h stack.h tcb.h threads.h
namespace.o: private CFLAGS += -fpic -ftls-model=initial-exec
namespace.o: private CPPFLAGS += -isystem . -D_GNU_SOURCE
namespace.o: namespace.h threads.h
plot.o: plot.h
repl.o: private CPPFLAGS += -D_GNU_SOURCE
repl.o: repl.h tcb.h
segprot.o: segprot.h plot.h
shared.o: private CFLAGS += -fpic
shared.o: private CPPFLAGS += -D_GNU_SOURCE
shared.o: shared.h namespace.h
tcb.o: private CFLAGS += -fpic -ftls-model=initial-exec
tcb.o: tcb.h threads.h
whitelist.o: private CPPFLAGS += -D_GNU_SOURCE
whitelist.o: whitelist.h config.h handle.h namespace.h

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

lib%.a: lib%.rlib
	$(AR) d $@ $(shell cp $< $@ && $(AR) t $@ | grep -v '\.o$$')

lib%.o: %.o
	$(LD) $(LDFLAGS) -r -o $@ $^ $(LDLIBS)
	if [ -e $*.abi ]; then $(OBJCOPY) --keep-global-symbols=$*.abi $@; fi

lib%.rlib: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS)" $(RUSTFLAGS) --crate-type rlib $< $(LDLIBS)
	if [ -e lib$*.o ]; then $(AR) rs $@ lib$*.o; fi

lib%.so: %.rs
	$(RUSTC) -Clink-args="$(LDFLAGS) -zdefs -ztext" $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic -Clinker=./cc $< $(LDLIBS)

lib%.so: %.o
	$(CC) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)
