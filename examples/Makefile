CARGO := cargo

CFLAGS     := -std=c99 -O2 -Wall -Wextra -Wpedantic
CARGOFLAGS := --release

.PHONY: recursive
recursive: date repltest
	cd cargo && $(CARGO) build $(CARGOFLAGS)
	$(MAKE) $(MAKEFLAGS) -C..
	$(MAKE) $(MAKEFLAGS) -Cmake

date:
date.o: CFLAGS += -fpic

.PHONY: clean
clean:
	$(RM) date repltest *.o cargo/Cargo.lock
	$(MAKE) -Cmake clean
	cd cargo && $(CARGO) clean
