CXX := c++
LD := $(CXX)
RUSTC := rustc

libdir := $(shell $(RUSTC) --print target-libdir)

override CPPFLAGS := -DBITVECTOR_SIZE=72 $(CPPFLAGS)
override CXXFLAGS := -std=c++14 -O2 -fpic -fno-rtti -Wall -Wextra -Wpedantic $(CXXFLAGS)
override LDFLAGS := -L$(libdir) $(LDFLAGS)
override LDLIBS := -lLLVM-$(shell $(libdir)/../bin/llc --version | sed -n 's/\s\+LLVM version \([^.]\+\)[^-]*\(.*\)/\1\2/p') -ldl $(LDLIBS)

libingerc.so:

.PHONY: clean
clean:
	$(RM) libingerc.*o

lib%.so: %.o
	$(LD) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)
