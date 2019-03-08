LN := ln -s

override CFLAGS  := -std=c99 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override LDFLAGS := -Wl,-R\$$ORIGIN $(LDFLAGS)
override LDLIBS  := decls.c $(LDLIBS)

ALL := bin_other bin_self lib_other.so lib_self.so

.PHONY: all
all: $(ALL)

bin_other: lib_self.so
lib_other.so: lib_self.so

%_other.s:
	$(LN) /dev/null $@

%_self.c: defns.c
	$(LN) $< $@

lib%.so: lib%.o
	$(CC) $(LDFLAGS) -shared -fpic -o $@ $^ $(LDLIBS)

.PHONY: distclean
distclean: clean
	$(RM) $(shell sed -n 's/\///p' .gitignore) *.so

.PHONY: clean
clean:
	$(RM) *.o
	find . -maxdepth 1 -type l -exec $(RM) {} \;
