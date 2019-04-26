ifndef LIBGOTCHA_PATH
LIBGOTCHA_PATH := $(dir $(lastword $(MAKEFILE_LIST)))
endif

ifndef ELFUTILS_PATH
ELFUTILS_PATH := /usr/lib/x86_64-linux-gnu/elfutils
endif

lib%.so: %.o $(LIBGOTCHA_PATH)/libgotcha.a
	$(CC) $(LDFLAGS) -shared -zdefs -zinitfirst -znodelete -ztext -L$(ELFUTILS_PATH) -Wl,-R$(ELFUTILS_PATH) -o $@ $^ $(LDLIBS) -Wl,--whole-archive $(LIBGOTCHA_PATH)/libgotcha.a -Wl,--no-whole-archive -lasm -ldl -lebl_x86_64 -lpthread -l$(LIBSTDRUST_SONAME)
