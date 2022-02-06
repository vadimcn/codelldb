use super::*;

cpp_class!(pub unsafe struct SBModuleSpec as "SBModuleSpec");

unsafe impl Send for SBModuleSpec {}

impl SBModuleSpec {
    pub fn new() -> SBModuleSpec {
        cpp!(unsafe [] -> SBModuleSpec as "SBModuleSpec" { return SBModuleSpec(); })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBModuleSpec*"] {
            self->Clear();
        })
    }
    pub fn file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModuleSpec*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetFileSpec();
        })
    }
    pub fn set_file_spec(&self, filespec: &SBFileSpec) {
        cpp!(unsafe [self as "SBModuleSpec*", filespec as "const SBFileSpec*"] {
            self->SetFileSpec(*filespec);
        })
    }
    pub fn platform_file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModuleSpec*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetPlatformFileSpec();
        })
    }
    pub fn set_platform_file_spec(&self, filespec: &SBFileSpec) {
        cpp!(unsafe [self as "SBModuleSpec*", filespec as "const SBFileSpec*"] {
            self->SetPlatformFileSpec(*filespec);
        })
    }
    pub fn symbol_file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModuleSpec*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetSymbolFileSpec();
        })
    }
    pub fn set_symbol_file_spec(&self, filespec: &SBFileSpec) {
        cpp!(unsafe [self as "SBModuleSpec*", filespec as "const SBFileSpec*"] {
            self->SetSymbolFileSpec(*filespec);
        })
    }
}

impl IsValid for SBModuleSpec {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBModuleSpec*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBModuleSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBModuleSpec*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
