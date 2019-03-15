LN := ln -s

override ASFLAGS  := $(ASFLAGS)
         CFLAGS_  := $(CFLAGS)
override CFLAGS    = -std=c99 -O2 -Wall -Wextra -Wpedantic $(if $(filter lib%.o,$@),-fpic) $(CFLAGS_)
override CPPFLAGS := $(CPPFLAGS)
override LDFLAGS  := -Wl,-R\$$ORIGIN $(LDFLAGS)
override LDLIBS   := $(LDLIBS)

ALL := tls_local_exec tls_initial_exec tls_local_dynamic tls_global_dynamic

.PHONY: all
all: $(ALL)

tls_local_exec: liblocal_exec.o
liblocal_exec.so:
libinitial_exec.so: libinitial_exec.o
liblocal_dynamic.so: liblocal_dynamic.o
libglobal_dynamic.so: libglobal_dynamic.o

libinitial_exec.o: private CFLAGS += -ftls-model=initial-exec
libglobal_dynamic.o: private CPPFLAGS += -Dglobal

entry.o: private CPPFLAGS += -D_GNU_SOURCE

empty.s:
	$(LN) /dev/null $@

.PHONY: distclean
distclean: clean
	$(RM) $(ALL) *.so

.PHONY: clean
clean:
	$(RM) *.o empty.s

lib%.c: local.c
	$(LN) $< $@

lib%.so: entry.o
	$(CC) $(LDFLAGS) -shared -fpic -o $@ $^ $(LDLIBS) -ldl

tls_%: empty.o lib%.so
	$(CC) $(LDFLAGS) -o $@ $^ $(LDLIBS)
