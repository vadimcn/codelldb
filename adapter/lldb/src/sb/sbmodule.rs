use super::*;

cpp_class!(pub unsafe struct SBModule as "SBModule");



impl SBModule {
    pub fn uuid_string(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBModule*"] -> *const c_char as "const char*" {
            return self->GetUUIDString();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModule*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetFileSpec();
        })
    }
    pub fn platform_file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModule*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetPlatformFileSpec();
        })
    }
    pub fn remote_install_file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModule*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetRemoteInstallFileSpec();
        })
    }
    pub fn symbol_file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBModule*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetSymbolFileSpec();
        })
    }
    pub fn object_header_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBModule*"] -> SBAddress as "SBAddress" {
            return self->GetObjectFileHeaderAddress();
        })
    }
    pub fn num_symbols(&self) -> u32 {
        cpp!(unsafe [self as "SBModule*"] -> u32 as "uint32_t" {
                return self->GetNumSymbols();
        })
    }
    pub fn symbol_at_index(&self, index: u32) -> SBSymbol {
        cpp!(unsafe [self as "SBModule*", index as "uint32_t"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbolAtIndex(index);
        })
    }
    pub fn symbols<'a>(&'a self) -> impl Iterator<Item = SBSymbol> + 'a {
        SBIterator::new(self.num_symbols(), move |index| self.symbol_at_index(index))
    }
    pub fn num_compile_units(&self) -> u32 {
        cpp!(unsafe [self as "SBModule*"] -> u32 as "uint32_t" {
                return self->GetNumCompileUnits();
        })
    }
    pub fn compile_unit_at_index(&self, index: u32) -> SBCompileUnit {
        cpp!(unsafe [self as "SBModule*", index as "uint32_t"] -> SBCompileUnit as "SBCompileUnit" {
            return self->GetCompileUnitAtIndex(index);
        })
    }
    pub fn compile_units<'a>(&'a self) -> impl Iterator<Item = SBCompileUnit> + 'a {
        SBIterator::new(self.num_compile_units(), move |index| self.compile_unit_at_index(index))
    }
}

impl IsValid for SBModule {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBModule*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBModule {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBModule*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
