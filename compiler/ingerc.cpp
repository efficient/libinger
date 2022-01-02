#include "X86InstrInfo.h"
#include "X86RegisterInfo.h"

#include "llvm/CodeGen/TargetInstrInfo.h"
#include "llvm/IR/LegacyPassManager.h"
#include "llvm/IR/Module.h"
#include "llvm/MC/MCContext.h"

#include <cstdlib>
#include <dlfcn.h>

using llvm::legacy::PassManager;
using llvm::DebugLoc;
using llvm::Function;
using llvm::FunctionType;
using llvm::GlobalValue;
using llvm::MachineBasicBlock;
using llvm::MachineFunction;
using llvm::MachineFunctionPass;
using llvm::MachineInstr;
using llvm::MachineInstrBuilder;
using llvm::MachineInstrBundleIterator;
using llvm::MCContext;
using llvm::MCSymbol;
using llvm::Pass;
using llvm::PassInfo;
using llvm::PassRegistry;
using llvm::PointerType;
using llvm::SmallVector;
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
			auto &cleanupBlock = *pad.LandingPadBlock;
			outs() << "landing pad: " << *pad.LandingPadLabel << '\n';

			auto dropCall = std::find_if(
				cleanupBlock.begin(),
				cleanupBlock.end(),
				std::bind(isCallTo, std::placeholders::_1, [](auto *fun) {
					return fun && fun->getName().contains("drop_in_place");
				})
			);

			auto *beginBlock = &mf.front();
			auto beginLoc = beginBlock->begin();

			auto endIt = mf.end();
			--endIt;
			if(&*endIt == &cleanupBlock)
				--endIt;

			auto *endBlock = &*endIt;
			auto endLoc = endBlock->end();

			if(dropCall == cleanupBlock.end()) {
				auto *sRetType = getFunctionSRetType(mf.getFunction());
				const Function *dtor = nullptr;
				if(sRetType)
					dtor = findDtor(*sRetType, mf);
				if(dtor) {
					auto argIndex = -1ul;
					auto ctor = findInst(mf, [&sRetType, &argIndex](auto &each) {
						auto *fun = getFunction(each);
						if(!fun)
							return false;
						return getFunctionSRetType(*fun, &argIndex) == sRetType;
					});
					assert(ctor);
					beginBlock = endBlock = (*ctor)->getParent();
					++*ctor;
					beginLoc = endLoc = *ctor;

					// Find the mov or lea before the constructor call.
					assert(argIndex == 0);
					--*ctor;
					--*ctor;
					while((*ctor)->getOperand(0).getReg() != llvm::X86::RDI) {
						assert(*ctor != (*ctor)->getParent()->begin());
						--*ctor;
					}

					auto move = cleanupBlock.begin();
					while(move != cleanupBlock.end() && !move->isMoveReg())
						++move;
					assert(move != cleanupBlock.end());

					// mov %rax, %rbp ; Save _Unwind_Context pointer.
					move->getOperand(0).setReg(llvm::X86::RBP);

					auto unwind = move;
					while(unwind != cleanupBlock.end() && !unwind->isCall())
						++unwind;
					assert(unwind != cleanupBlock.end());

					// (mov|lea) ???, %rdi ; Pass victim to destructor.
					addInst(
						cleanupBlock,
						unwind,
						(*ctor)->getOpcode(),
						[&ctor](auto &inst, auto &) {
							inst.addReg(llvm::X86::RDI);

							auto &src = (*ctor)->getOperand(1);
							if(src.isReg())
								inst.addReg(src.getReg());
							else
								inst.cloneMemRefs(**ctor);
						}
					);

					dropCall = addInst(
						cleanupBlock,
						unwind,
						unwind->getOpcode(),
						[&dtor](auto &inst, auto &) {
							inst.addGlobalAddress(dtor);
						}
					);

					// mov %rbp, %rdi ; Pass _Unwind_Context pointer.
					addInst(
						cleanupBlock,
						unwind,
						move->getOpcode(),
						[](auto &inst, auto &) {
							inst.addReg(llvm::X86::RDI);
							inst.addReg(llvm::X86::RBP);
						}
					);
				}
			}

			if(dropCall != cleanupBlock.end()) {
				if(getEpilogueFunction()) {
					auto epilogue = findInst(mf, [](auto &inst) {
						return inst.isCall()
							&& getFunction(inst)->getName() == getEpilogueFunction();
					});
					if(epilogue) {
						++*epilogue;
						addInst(
							*(*epilogue)->getParent(),
							*epilogue,
							llvm::X86::NOOP,
							[](auto &, auto &) {}
						);
					}
				}

				auto &beginLabel = getOrCreateLabel(
					mf.getOrCreateLandingPadInfo(&cleanupBlock).BeginLabels,
					*beginBlock,
					beginLoc,
					changed
				);

				auto &endLabel = getOrCreateLabel(
					mf.getOrCreateLandingPadInfo(&cleanupBlock).EndLabels,
					*endBlock,
					endLoc,
					changed
				);
				outs() << "bounding labels: " << beginLabel << ' ' << endLabel << '\n';

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
	static MachineInstr &addInst(
		MachineBasicBlock &block,
		MachineInstrBundleIterator<MachineInstr> pos,
		unsigned opcode,
		std::function<void(const MachineInstrBuilder &, MCContext &)> operands
	) {
		auto &mf = *block.getParent();
		auto &info = mf.getSubtarget().getInstrInfo()->get(opcode);
		auto &inst = *mf.CreateMachineInstr(info, DebugLoc());
		MachineInstrBuilder build(mf, inst);
		operands(build, mf.getContext());
		block.insert(pos, &inst);
		return inst;
	}

	static const Function *findDtor(const Type &type, const MachineFunction &fun) {
		for(auto &defn : *fun.getFunction().getParent()) {
			if(defn.getName().contains("drop_in_place")) {
				const auto &params = defn.getFunctionType()->params();
				if(
					params.size()
					&& params.front()->isPointerTy()
					&& static_cast<PointerType *>(params.front())->getElementType() == &type
				)
					return &defn;
			}
		}
		return nullptr;
	}

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

	static const char *getEpilogueFunction() {
		static const char *epilogue = nullptr;
		static bool memo = false;
		if(!memo) {
			epilogue = getenv("INGERC_EPILOGUE");
			memo = true;
		}
		return epilogue;
	}

	static const Function *getFunction(const MachineInstr &call) {
		if(!call.isCall())
			return nullptr;

		return static_cast<const Function *>(call.getOperand(0).getGlobal());
	}

	static const Type *getFunctionSRetType(const Function &fun, size_t *param = nullptr) {
		auto params = fun.getFunctionType()->params();
		for(auto index = 0u; index != params.size(); ++index) {
			auto *type = fun.getParamStructRetType(index);
			if(type) {
				if(param)
					*param = index;
				return type;
			}
		}
		return nullptr;
	}

	static const FunctionType &getFunctionType(const MachineInstr &call) {
		assert(call.isCall());

		auto &fun = *call.getOperand(0).getGlobal();
		auto &funType = *fun.getType()->getElementType();
		assert(funType.isFunctionTy());

		return static_cast<FunctionType &>(funType);
	}

	static MCSymbol &getOrCreateLabel(
		SmallVector<MCSymbol *, 1> &labels,
		MachineBasicBlock &block,
		MachineInstrBundleIterator<MachineInstr> pos,
		bool &changed
	) {
		if(labels.size())
			return *labels.front();

		auto &inst = addInst(
			block,
			pos,
			llvm::TargetOpcode::EH_LABEL,
			[](auto &inst, auto &syms) {
				inst.addSym(syms.createTempSymbol());
			}
		);
		auto &label = *inst.getOperand(0).getMCSymbol();
		labels.push_back(&label);
		changed = true;
		return label;
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
