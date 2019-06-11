CFLAGS    := -std=c99 -O2 -Wall -Wextra -Wpedantic
RUSTFLAGS := --edition 2018 -O

.PHONY: all
all: libctime.so librstime.so

.PHONY: clean
clean:
	$(RM) *.a *.o *.rlib *.so

include ../../libgotcha.mk
