ifndef LIBGOTCHA_PATH
LIBGOTCHA_PATH := $(dir $(lastword $(MAKEFILE_LIST)))
endif

ifndef ELFUTILS_PATH
ELFUTILS_PATH := /usr/lib/x86_64-linux-gnu/elfutils
endif

ifndef RUSTC
RUSTC := rustc
endif

ifndef RUSTFLAGS
RUSTFLAGS := -O
endif

lib%.so: %.o libgotcha.a
	$(CC) $(LDFLAGS) -shared -zdefs -zinitfirst -znodelete -znoexecstack -ztext -static-libgcc -L. -L$(ELFUTILS_PATH) -Wl,-R$(ELFUTILS_PATH) -o $@ $< $(LDLIBS) -Wl,--whole-archive -lgotcha -Wl,--no-whole-archive -l$(LIBSTDRUST_SONAME) -lasm -lc -ldl -lebl_x86_64 -lpthread

lib%.so: %.rs libgotcha.rlib
	$(RM) libgotcha.so
	$(RUSTC) $(RUSTFLAGS) --crate-type dylib -Clink-args="$(LDFLAGS) -zdefs -zinitfirst -znodelete -ztext -Wl,-zlazy -L$(ELFUTILS_PATH) -Wl,-R$(ELFUTILS_PATH) -lasm -lebl_x86_64" -Cprefer-dynamic -L. $<

libgotcha.%: $(LIBGOTCHA_PATH)/libgotcha.%
	cp $< $@
