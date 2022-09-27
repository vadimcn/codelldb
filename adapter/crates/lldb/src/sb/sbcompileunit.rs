use super::*;

cpp_class!(pub unsafe struct SBCompileUnit as "SBCompileUnit");

unsafe impl Send for SBCompileUnit {}

impl SBCompileUnit {
    pub fn file_spec(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBCompileUnit*"] -> SBFileSpec as "SBFileSpec" {
            return self->GetFileSpec();
        })
    }
    pub fn num_line_entries(&self) -> u32 {
        cpp!(unsafe [self as "SBCompileUnit*"] -> u32 as "uint32_t" {
            return self->GetNumLineEntries();
        })
    }
    pub fn line_entry_at_index(&self, index: u32) -> SBLineEntry {
        cpp!(unsafe [self as "SBCompileUnit*", index as "uint32_t"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntryAtIndex(index);
        })
    }
    pub fn line_entries<'a>(&'a self) -> impl Iterator<Item = SBLineEntry> + 'a {
        SBIterator::new(self.num_line_entries(), move |index| self.line_entry_at_index(index))
    }
    pub fn language(&self) -> LanguageType {
        cpp!(unsafe [self as "SBCompileUnit*"] -> c_uint as "unsigned int" {
            return self->GetLanguage();
        })
        .into()
    }
}

impl IsValid for SBCompileUnit {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBCompileUnit*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBCompileUnit {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBCompileUnit*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
