override ASFLAGS := $(ASFLAGS)
override LDFLAGS := $(LDFLAGS)
override LDLIBS := $(LDLIBS)

lib%.a: %.o
	$(AR) rs $@ $^
