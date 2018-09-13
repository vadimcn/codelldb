use super::*;

cpp_class!(pub unsafe struct SBAddress as "SBAddress");

unsafe impl Send for SBAddress {}

impl SBAddress {
    pub fn from_load_address(addr: u64, target: &SBTarget) -> Self {
        cpp!(unsafe [addr as "addr_t", target as "SBTarget*"] -> SBAddress as "SBAddress" {
            return SBAddress(addr, *target);
        })
    }
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBAddress*"] -> bool as "bool" {
            return self->IsValid();
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
    pub fn offset(&self) -> usize {
        cpp!(unsafe [self as "SBAddress*"] -> usize as "size_t" {
            return self->GetOffset();
        })
    }
    pub fn line_entry(&self) -> Option<SBLineEntry> {
        let line_entry = cpp!(unsafe [self as "SBAddress*"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntry();
        });
        if line_entry.is_valid() {
            Some(line_entry)
        } else {
            None
        }
    }
    pub fn symbol(&self) -> Option<SBSymbol> {
        let symbol = cpp!(unsafe [self as "SBAddress*"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbol();
        });
        if symbol.is_valid() {
            Some(symbol)
        } else {
            None
        }
    }
    pub fn get_description(&self, description: &mut SBStream) -> bool {
        cpp!(unsafe [self as "SBAddress*", description as "SBStream*"] -> bool as "bool" {
            return self->GetDescription(*description);
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
