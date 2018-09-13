use super::*;

cpp_class!(pub unsafe struct SBInstruction as "SBInstruction");

unsafe impl Send for SBInstruction {}

impl SBInstruction {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBInstruction*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
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
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn operands(&self, target: &SBTarget) -> &str {
        let target = target.clone();
        let ptr = cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> *const c_char as "const char*" {
            return self->GetOperands(target);
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn comment(&self, target: &SBTarget) -> &str {
        let target = target.clone();
        let ptr = cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> *const c_char as "const char*" {
            return self->GetComment(target);
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBInstruction*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn data(&self, target: &SBTarget) -> SBData {
        let target = target.clone();
        cpp!(unsafe [self as "SBInstruction*", target as "SBTarget"] -> SBData as "SBData" {
            return self->GetData(target);
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
