#ifndef LLVM_ADT_BITVECTOR_H
#define BitVector RealBitVector
#include "llvm/ADT/BitVector.h"
#undef BitVector

#if !BITVECTOR_SIZE
# error "Build with -DBITVECTOR_SIZE=<#>, where <#> matches that of llc and libLLVM-*.so!"
#endif

namespace llvm {
struct BitVector {
	explicit BitVector(unsigned bits) {
		new (this) RealBitVector(bits);
	}

	BitVector &reset();
	BitVector &reset(unsigned);
	BitVector &reset(const BitVector &);
	BitVector &set(unsigned);
	bool &test(const BitVector &) const;
	BitVector &operator|=(const BitVector &);
	bool operator[](unsigned) const;

private:
	unsigned char pad[BITVECTOR_SIZE];
};
}

#endif
