use super::*;

cpp_class!(pub unsafe struct SBSymbolContext as "SBSymbolContext");

unsafe impl Send for SBSymbolContext {}

impl SBSymbolContext {
    pub fn module(&self) -> SBModule {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBModule as "SBModule" {
            return self->GetModule();
        })
    }
    pub fn line_entry(&self) -> SBLineEntry {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntry();
        })
    }
    pub fn symbol(&self) -> SBSymbol {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbol();
        })
    }
    pub fn get_description(&self, description: &mut SBStream) -> bool {
        cpp!(unsafe [self as "SBSymbolContext*", description as "SBStream*"] -> bool as "bool" {
            return self->GetDescription(*description);
        })
    }
}

impl IsValid for SBSymbolContext {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBSymbolContext*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBSymbolContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBSymbolContext*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
