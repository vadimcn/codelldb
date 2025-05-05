use super::*;

cpp_class!(pub unsafe struct SBDeclaration as "SBDeclaration");

impl SBDeclaration {
    pub fn filespec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBDeclaration*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetFileSpec();
        })
    }

    pub fn line(&self) -> u32 {
        cpp!(unsafe [self as "SBDeclaration*"] -> u32 as "uint32_t" {
            return self->GetLine();
        })
    }

    pub fn column(&self) -> u32 {
        cpp!(unsafe [self as "SBDeclaration*"] -> u32 as "uint32_t" {
            return self->GetColumn();
        })
    }
}

impl IsValid for SBDeclaration {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBDeclaration*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBDeclaration {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBDeclaration*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
