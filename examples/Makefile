CFLAGS := -std=c99 -O2 -Wall -Wextra -Wpedantic

.PHONY: recursive
recursive: date
	$(MAKE) $(MAKEFLAGS) -Cmake

date:
date.o: CFLAGS += -fpic

.PHONY: clean
clean:
	$(RM) date *.o
	$(MAKE) -Cmake clean
