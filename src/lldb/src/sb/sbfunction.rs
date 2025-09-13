use super::*;

cpp_class!(pub unsafe struct SBFunction as "SBFunction");

unsafe impl Send for SBFunction {}

impl SBFunction {
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBFunction*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn display_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBFunction*"] -> *const c_char as "const char*" {
            return self->GetDisplayName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn mangled_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBFunction*"] -> *const c_char as "const char*" {
            return self->GetMangledName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn start_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBFunction*"] -> SBAddress as "SBAddress" {
            return self->GetStartAddress();
        })
    }
    pub fn end_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBFunction*"] -> SBAddress as "SBAddress" {
            return self->GetEndAddress();
        })
    }
}

impl PartialEq for SBFunction {
    fn eq(&self, other: &Self) -> bool {
        cpp!(unsafe [self as "SBFunction*", other as "SBFunction*"] -> bool as "bool" {
            return *self == *other;
        })
    }
}

impl IsValid for SBFunction {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBFunction*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBFunction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBFunction*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
