CFLAGS    := -std=c99 -O2 -Wall -Wextra -Wpedantic
RUSTFLAGS := --edition 2018 -O

date:
date.o: CFLAGS += -fpic

.PHONY: clean
clean:
	$(RM) date *.a *.o *.rlib *.so

include ../libgotcha.mk
