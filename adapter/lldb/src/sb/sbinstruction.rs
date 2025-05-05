use super::*;

cpp_class!(pub unsafe struct SBInstruction as "SBInstruction");



impl SBInstruction {
    pub fn address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBInstruction*"] -> SBAddress as "SBAddress" {
            return self->GetAddress();
        })
    }
    pub fn mnemonic(&self, target: &SBTarget) -> &str {
        let target = target.clone();
        let ptr = cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> *const c_char as "const char*" {
            return self->GetMnemonic(target);
        });
        unsafe { get_str(ptr) }
    }
    pub fn operands(&self, target: &SBTarget) -> &str {
        let target = target.clone();
        let ptr = cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> *const c_char as "const char*" {
            return self->GetOperands(target);
        });
        unsafe { get_str(ptr) }
    }
    pub fn comment(&self, target: &SBTarget) -> &str {
        let target = target.clone();
        let ptr = cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> *const c_char as "const char*" {
            return self->GetComment(target);
        });
        unsafe { get_str(ptr) }
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBInstruction*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn data(&self, target: &SBTarget) -> SBDataOwned {
        let target = target.clone();
        cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> SBDataOwned as "SBData" {
            return self->GetData(target);
        })
    }
    pub fn control_flow_kind(&self, target: &SBTarget) -> InstructionControlFlowKind {
        cpp!(unsafe [self as "SBInstruction*", target as "SBTarget*"] -> InstructionControlFlowKind as "InstructionControlFlowKind" {
            return self->GetControlFlowKind(*target);
        })
    }
}

impl IsValid for SBInstruction {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBInstruction*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBInstruction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBInstruction*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Default, Debug, FromPrimitive)]
#[repr(u32)]
pub enum InstructionControlFlowKind {
    /// The instruction could not be classified.
    #[default]
    Unknown = 0,
    /// The instruction is something not listed below, i.e. it's a sequential
    /// instruction that doesn't affect the control flow of the program.
    Other,
    /// The instruction is a near (function) call.
    Call,
    /// The instruction is a near (function) return.
    Return,
    /// The instruction is a near unconditional jump.
    Jump,
    /// The instruction is a near conditional jump.
    CondJump,
    /// The instruction is a call-like far transfer.
    /// E.g. SYSCALL, SYSENTER, or FAR CALL.
    FarCall,
    /// The instruction is a return-like far transfer.
    /// E.g. SYSRET, SYSEXIT, IRET, or FAR RET.
    FarReturn,
    /// The instruction is a jump-like far transfer.
    /// E.g. FAR JMP.
    FarJump,
}
