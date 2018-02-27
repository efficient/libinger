DEBUG :=
PRIVATE :=

CARGO := cargo

override CARGOFLAGS := $(if $(DEBUG),,--release) $(CARGOFLAGS)
override DOCFLAGS := $(if $(PRIVATE),--document-private-items,) $(DOCFLAGS)
override RUSTFLAGS := $(RUSTFLAGS)
TESTFLAGS := --nocapture

ifneq ($(DEBUG),)
OUTDIR := target/debug
else
OUTDIR := target/release
endif

.PHONY: help
help:
	@echo "Supported targets:"
	@echo "  Development:"
	@echo "    test [DEBUG=y]"
	@echo "    check"
	@echo
	@echo "  Documentation:"
	@echo "    doc [PRIVATE=y]"
	@echo
	@echo "  Crate:"
	@echo "    build (default: dylib)"
	@echo "    rlib  [DEBUG=y]"
	@echo "    dylib [DEBUG=y]"
	@echo
	@echo "  Native:"
	@echo "    libinger.a [DEBUG=y]"
	@echo "    libinger.so [DEBUG=y]"
	@echo
	@echo "  Cleanup:"
	@echo "    clean"

.PHONY: test
test:
	$(CARGO) test $(CARGOFLAGS) -- $(TESTFLAGS)

.PHONY: check
check:
	$(CARGO) check $(filter-out --release,$(CARGOFLAGS))

.PHONY: doc
doc:
	$(CARGO) doc $(CARGOFLAGS)
	$(CARGO) rustdoc $(CARGOFLAGS) -- $(DOCFLAGS)

.PHONY: build
build: dylib

.PHONY: rlib
rlib:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type rlib
	mv $(OUTDIR)/deps/libinger.rlib $(OUTDIR)/libinger.rlib || true

.PHONY: dylib
dylib:
	$(CARGO) build $(CARGOFLAGS)

.PHONY: libinger.a
libinger.a:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type staticlib
	mv $(OUTDIR)/deps/libinger.a $@ || true

.PHONY: libinger.so
libinger.so:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type cdylib -Clink-arg=-Wl,-h,$@
	mv $(OUTDIR)/deps/libinger-*.so $@ || true

.PHONY: clean
clean:
	git clean -fdX
