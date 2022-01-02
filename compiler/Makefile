CXX := c++
LD := $(CXX)
SED := sed
TBLGEN := llvm-tblgen

override CPPFLAGS := -DGET_INSTRINFO_ENUM -DGET_REGINFO_ENUM $(CPPFLAGS)
override CXXFLAGS := -std=c++17 -O2 -fpic -fno-rtti -Wall -Wextra -Wpedantic $(CXXFLAGS)
override LDLIBS := -lLLVM-13 -ldl $(LDLIBS)
override TBLGENFLAGS := -I$(LLVM_SOURCE_DIR)/include $(TBLGENFLAGS)

libingerc.so: X86InstrInfo.h X86RegisterInfo.h

X86InstrInfo.h: $(LLVM_SOURCE_DIR)/lib/Target/X86/X86.td
	$(TBLGEN) -gen-instr-info $(TBLGENFLAGS) -I$(dir $<) -o $@ $<

X86RegisterInfo.h: $(LLVM_SOURCE_DIR)/lib/Target/X86/X86.td
	$(TBLGEN) -gen-register-info $(TBLGENFLAGS) -I$(dir $<) -o $@ $<
	$(SED) -i 's/ : \S\+\( {\)/\1/' $@

.PHONY: clean
clean:
	$(RM) libingerc.*o

lib%.so: %.o
	$(LD) $(LDFLAGS) -shared -zdefs -ztext -o $@ $^ $(LDLIBS)
