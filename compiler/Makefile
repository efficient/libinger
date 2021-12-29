CXX := c++
LD := $(CXX)

override CXXFLAGS := -std=c++14 -O2 -fpic -fno-rtti -Wall -Wextra -Wpedantic $(CXXFLAGS)
override LDLIBS := -lLLVM-13 -ldl $(LDLIBS)

libingerc.so:

.PHONY: clean
clean:
	$(RM) libingerc.*o

lib%.so: %.o
	$(LD) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)
