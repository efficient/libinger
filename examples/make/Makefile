CFLAGS    := -std=c99 -O2 -Wall -Wextra -Wpedantic
RUSTFLAGS := --edition 2018 -O

.PHONY: all
all: libctime.so librstime.so libctls.so librstls.so

.PHONY: clean
clean:
	$(RM) *.a *.o *.rlib *.so

# This is the only essential line.  Note that you must have already built at least libgotcha.mk and
# libgotcha.rlib in the main source tree before using the client library's build system.
include ../../libgotcha.mk
