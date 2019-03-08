LN := ln -s

override CFLAGS  := -std=c99 -O2 -Wall -Wextra -Wpedantic $(CFLAGS)
override LDFLAGS := -Wl,-R\$$ORIGIN $(LDFLAGS)
         LDLIBS_ := $(LDLIBS)
override LDLIBS   = $(if $(filter %_interpose %_interpose.so load_%,$@),,decls.c) $(LDLIBS_)

override GREPFLAGS := '\<[a-z]\+_[a-z]\+\>\S*$$' $(GREPFLAGS)
override SORTFLAGS := -k2 $(SORTFLAGS)

ALL := bin_other pic_other bin_self bin_interpose lib_other.so lib_self.so lib_interpose.so

.PHONY: all
all: $(ALL)

.PHONY: r
r: $(ALL:=.R)

.PHONY: t
t: $(ALL:=.T)

.PHONY: load
load: load_other load_self load_interpose

bin_other: lib_self.so
lib_other.so: lib_self.so

pic_other: lib_self.so
pic_other: private LDFLAGS += -fpic

bin_interpose: lib_self.so
lib_interpose.so: lib_self.so
lib_interpose.so: private LDFLAGS += -umain

load_other: load_other.s lib_other.so
load_self: load_self.s lib_self.so
load_interpose: load_interpose.s lib_interpose.so
load_interpose: private LDFLAGS += -Wl,--unresolved-symbols=ignore-all

%_other.s:
	$(LN) /dev/null $@

%_self.c: defns.c
	$(LN) $< $@

%_interpose.c: defns.c
	$(LN) $< $@

load_%.s:
	$(LN) /dev/null $@

lib%.so: lib%.o
	$(CC) $(LDFLAGS) -shared -fpic -o $@ $^ $(LDLIBS)

%.R: %
	objdump -R $< | grep $(GREPFLAGS) | sort $(SORTFLAGS) >$@

%.T: %
	objdump -T $< | grep $(GREPFLAGS) | sort $(SORTFLAGS) >$@

.PHONY: distclean
distclean: clean
	$(RM) $(shell sed -n 's/\///p' .gitignore) *.so

.PHONY: clean
clean:
	$(RM) *.o *.R *.T
	find . -maxdepth 1 -type l -exec $(RM) {} \;
