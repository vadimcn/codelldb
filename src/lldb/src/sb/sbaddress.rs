use super::*;

cpp_class!(pub unsafe struct SBAddress as "SBAddress");

unsafe impl Send for SBAddress {}

impl SBAddress {
    pub fn from_load_address(addr: u64, target: &SBTarget) -> Self {
        cpp!(unsafe [addr as "addr_t", target as "SBTarget*"] -> SBAddress as "SBAddress" {
            return SBAddress(addr, *target);
        })
    }
    pub fn file_address(&self) -> usize {
        cpp!(unsafe [self as "SBAddress*"] -> usize as "size_t" {
            return self->GetFileAddress();
        })
    }
    pub fn load_address(&self, target: &SBTarget) -> u64 {
        cpp!(unsafe [self as "SBAddress*", target as "SBTarget*"] -> u64 as "uint64_t" {
            return self->GetLoadAddress(*target);
        })
    }
    pub fn section(&self) -> Option<SBSection> {
        cpp!(unsafe [self as "SBAddress*"] -> SBSection as "SBSection" {
            return self->GetSection();
        })
        .check()
    }
    pub fn offset(&self) -> usize {
        cpp!(unsafe [self as "SBAddress*"] -> usize as "size_t" {
            return self->GetOffset();
        })
    }
    pub fn line_entry(&self) -> Option<SBLineEntry> {
        cpp!(unsafe [self as "SBAddress*"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntry();
        })
        .check()
    }
    pub fn symbol(&self) -> Option<SBSymbol> {
        cpp!(unsafe [self as "SBAddress*"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbol();
        })
        .check()
    }
    pub fn function(&self) -> Option<SBFunction> {
        cpp!(unsafe [self as "SBAddress*"] -> SBFunction as "SBFunction" {
            return self->GetFunction();
        })
        .check()
    }
    pub fn module(&self) -> Option<SBModule> {
        cpp!(unsafe [self as "SBAddress*"] -> SBModule as "SBModule" {
            return self->GetModule();
        })
        .check()
    }
    /// Modifies object in-place
    pub fn add_offset(&mut self, offset: u64) -> bool {
        cpp!(unsafe [self as "SBAddress*", offset as "int64_t"] -> bool as "bool" {
            return self->OffsetAddress(offset);
        })
    }
}

impl IsValid for SBAddress {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBAddress*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBAddress*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
