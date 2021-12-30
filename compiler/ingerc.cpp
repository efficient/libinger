#include "llvm/ADT/BitVector.h"
#include "llvm/CodeGen/MachineFunctionPass.h"
#include "llvm/IR/GlobalValue.h"
#include "llvm/IR/LegacyPassManager.h"

#include <dlfcn.h>

using llvm::legacy::PassManager;
using llvm::FunctionType;
using llvm::GlobalValue;
using llvm::MachineFunction;
using llvm::MachineFunctionPass;
using llvm::MachineInstr;
using llvm::MachineInstrBundleIterator;
using llvm::Pass;
using llvm::PassInfo;
using llvm::PassRegistry;
using llvm::Type;
using llvm::callDefaultCtor;
using llvm::outs;

namespace {
struct IngerCancel: public MachineFunctionPass {
	virtual bool runOnMachineFunction(MachineFunction &mf) override {
		if(mf.getName().contains("drop_in_place"))
			return false;
		outs() << "FUNCTION: " << mf.getName() << '\n';

		auto changed = false;
		for(auto &pad : mf.getLandingPads()) {
			auto &beginLabel = *pad.BeginLabels.front();
			auto &endLabel = *pad.EndLabels.front();
			outs() << "landing pad: " << beginLabel << ' ' << endLabel << '\n';

			auto &cleanupBlock = *pad.LandingPadBlock;
			outs() << cleanupBlock << '\n';

			auto dropCall = std::find_if(
				cleanupBlock.begin(),
				cleanupBlock.end(),
				std::bind(isCallTo, std::placeholders::_1, [](auto *fun) {
					return fun && fun->getName().contains("drop_in_place");
				})
			);
			if(dropCall != cleanupBlock.end()) {
				auto &dropFun = *dropCall->getOperand(0).getGlobal();
				outs() << "dropFun: " << dropFun.getName() << '\n';

				auto &dropType = *getFunctionType(*dropCall).params().front();
				outs() << "paramType: " << dropType << '\n';

				auto labelFinder = [](auto &label) {
					return [&label](auto &each) {
						return std::any_of(
							each.operands_begin(),
							each.operands_end(),
							[&label](auto &each) {
								return each.isMCSymbol()
									&& each.getMCSymbol() == &label;
							}
						);
					};
				};

				auto beginInst = findInst(mf, labelFinder(beginLabel));
				assert(beginInst);
				outs() << "beginInst: " << **beginInst << '\n';

				auto endInst = findInst(mf, labelFinder(endLabel));
				assert(endInst);
				outs() << "endInst: " << **endInst << '\n';

				auto prevInst = (*beginInst)->getPrevNode();
				assert(prevInst);
				if(!isCallUsing(*prevInst, dropType)) {
					auto nextInst = this->nextInst(*beginInst);
					while(
						nextInst
						&& !(*nextInst)->isCall()
						&& *nextInst != endInst
					)
						nextInst = this->nextInst(*nextInst);
					if(nextInst && isCallUsing(**nextInst, dropType)) {
						outs() << "nextInst: " << **nextInst << '\n';

						auto *movedInst = (*beginInst)->removeFromParent();
						(*nextInst)->getParent()->insertAfter(
							*nextInst,
							movedInst
						);
						changed = true;
					}
				}

				auto nextInst = this->nextInst(*endInst);
				while(
					nextInst
					&& !isCallTo(**nextInst, [&dropFun](auto *fun) {
						return fun == &dropFun;
					})
					&& !(*nextInst)->isCFIInstruction()
					&& !(*nextInst)->isReturn()
				)
					nextInst = this->nextInst(*nextInst);
				assert(nextInst);
				outs() << "nextInst: " << **nextInst << '\n';

				if((*nextInst)->isCFIInstruction())
					--*nextInst;

				auto *movedInst = (*endInst)->removeFromParent();
				(*nextInst)->getParent()->insert(*nextInst, movedInst);
				changed = true;
			}
		}

		return changed;
	}

	static char ID;
	IngerCancel(): MachineFunctionPass(ID) {}

private:
	static std::optional<MachineInstrBundleIterator<MachineInstr>> findInst(
		MachineFunction &mf,
		std::function<bool(const MachineInstr &)> pred
	) {
		for(auto &block : mf) {
			auto inst = std::find_if(block.begin(), block.end(), pred);
			if(inst != block.end())
				return std::optional(inst);
		}
		return {};
	}

	static const FunctionType &getFunctionType(const MachineInstr &call) {
		assert(call.isCall());

		auto &fun = *call.getOperand(0).getGlobal();
		auto &funType = *fun.getType()->getElementType();
		assert(funType.isFunctionTy());

		return static_cast<FunctionType &>(funType);
	}

	static bool isCallTo(
		const MachineInstr &inst,
		std::function<bool(const GlobalValue *)> name
	) {
		auto &operand = inst.getOperand(0);
		const GlobalValue *fun = nullptr;
		if(operand.isGlobal())
			fun = operand.getGlobal();
		// else it's an indirect call so we cannot know the function statically
		return inst.isCall() && name(fun);
	}

	static bool isCallUsing(const MachineInstr &inst, const Type &type) {
		if(!inst.isCall())
			return false;

		auto &funType = getFunctionType(inst);
		return funType.getReturnType() == &type
			|| std::any_of(
				funType.param_begin(),
				funType.param_end(),
				[&type](auto &each) {
					return each == &type;
				}
			);
	}

	static std::optional<MachineInstrBundleIterator<MachineInstr>> nextInst(
		MachineInstrBundleIterator<MachineInstr> inst
	) {
		auto &block = *inst->getParent();
		auto nextInst = inst;
		++nextInst;
		if(nextInst != block.end())
			return std::optional(nextInst);

		auto *nextBlock = inst->getParent()->getNextNode();
		if(nextBlock)
			return std::optional(nextBlock->begin());

		return {};
	}
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