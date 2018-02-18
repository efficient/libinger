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
	@echo "    test"
	@echo "    check"
	@echo
	@echo "  Documentation:"
	@echo "    doc [PRIVATE=y]"
	@echo
	@echo "  Crate:"
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

.PHONY: rlib
rlib:
	$(CARGO) build $(CARGOFLAGS)

.PHONY: dylib
dylib:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type dylib -Cprefer-dynamic
	mv $(OUTDIR)/deps/libinger*.so $(OUTDIR)/libinger.so

.PHONY: libinger.a
libinger.a:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type staticlib
	mv $(OUTDIR)/deps/libinger*.a $@

.PHONY: libinger.so
libinger.so:
	$(CARGO) rustc $(CARGOFLAGS) -- $(RUSTFLAGS) --crate-type cdylib -Clink-arg=-Wl,-h,$@
	mv $(OUTDIR)/deps/libinger*.so $@

.PHONY: clean
clean:
	git clean -fdX
