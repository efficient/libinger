#include "llvm/ADT/BitVector.h"
#include <llvm/CodeGen/MachineFunctionPass.h>
#include <llvm/IR/LegacyPassManager.h>

#include <dlfcn.h>

using llvm::legacy::PassManager;
using llvm::MachineFunction;
using llvm::MachineFunctionPass;
using llvm::Pass;
using llvm::PassInfo;
using llvm::PassRegistry;
using llvm::callDefaultCtor;
using llvm::outs;

namespace {
struct IngerCancel: public MachineFunctionPass {
	virtual bool runOnMachineFunction(MachineFunction &mf) override {
		outs() << "runOnMachineFunction(" << mf.getName() << ")\n";
		return false;
	}

	static char ID;
	IngerCancel(): MachineFunctionPass(ID) {}
};
}

char IngerCancel::ID;

typedef void (*PassManager_add_t)(PassManager *, Pass *);
static PassManager_add_t _ZN4llvm6legacy11PassManager3addEPNS_4PassE;

static void add(PassManager *pm, Pass *p) {
	if(p->getPassName() == "X86 Assembly Printer")
		_ZN4llvm6legacy11PassManager3addEPNS_4PassE(pm, new IngerCancel());
	_ZN4llvm6legacy11PassManager3addEPNS_4PassE(pm, p);
}

extern "C" {
PassManager_add_t _ZTVN4llvm6legacy11PassManagerE[5];
}

namespace llvm {
void initializeIngerCancelPass(PassRegistry &);
}

extern "C" void LLVMInitializeX86Target() {
	void (*LLVMInitializeX86Target)() = (void (*)()) dlsym(RTLD_NEXT, "LLVMInitializeX86Target");
	LLVMInitializeX86Target();

	PassRegistry &pr = *PassRegistry::getPassRegistry();
	initializeIngerCancelPass(pr);

	_ZN4llvm6legacy11PassManager3addEPNS_4PassE = (PassManager_add_t)
		dlsym(RTLD_NEXT, "_ZN4llvm6legacy11PassManager3addEPNS_4PassE");
	const PassManager_add_t *vtable = (PassManager_add_t *)
		dlsym(RTLD_NEXT, "_ZTVN4llvm6legacy11PassManagerE");
	for(
		size_t index = 0;
		index != sizeof _ZTVN4llvm6legacy11PassManagerE / sizeof *_ZTVN4llvm6legacy11PassManagerE;
		++index
	)
		if(vtable[index] == _ZN4llvm6legacy11PassManager3addEPNS_4PassE)
			_ZTVN4llvm6legacy11PassManagerE[index] = add;
		else
			_ZTVN4llvm6legacy11PassManagerE[index] = vtable[index];
}

INITIALIZE_PASS(IngerCancel, "llc", "IngerCancel", false, false)
