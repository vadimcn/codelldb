use super::*;

cpp_class!(pub unsafe struct SBLineEntry as "SBLineEntry");

unsafe impl Send for SBLineEntry {}

impl SBLineEntry {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBLineEntry*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn line(&self) -> u32 {
        cpp!(unsafe [self as "SBLineEntry*"] -> u32 as "uint32_t" {
            return self->GetLine();
        })
    }
    pub fn column(&self) -> u32 {
        cpp!(unsafe [self as "SBLineEntry*"] -> u32 as "uint32_t" {
            return self->GetColumn();
        })
    }
    pub fn file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBLineEntry*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetFileSpec();
        })
    }
    pub fn start_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBLineEntry*"] -> SBAddress as "SBAddress" {
            return self->GetStartAddress();
        })
    }
    pub fn end_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBLineEntry*"] -> SBAddress as "SBAddress" {
            return self->GetEndAddress();
        })
    }
}

impl fmt::Debug for SBLineEntry {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBLineEntry*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
