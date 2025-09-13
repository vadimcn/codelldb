use super::*;

cpp_class!(pub unsafe struct SBSymbolContextList as "SBSymbolContextList");

unsafe impl Send for SBSymbolContextList {}

impl SBSymbolContextList {
    pub fn new() -> SBSymbolContextList {
        cpp!(unsafe [] -> SBSymbolContextList as "SBSymbolContextList" { return SBSymbolContextList(); })
    }
    pub fn len(&self) -> usize {
        cpp!(unsafe [self as "SBSymbolContextList*"] -> usize as "size_t" {
            return self->GetSize();
        })
    }
    pub fn clear(&mut self) {
        cpp!(unsafe [self as "SBSymbolContextList*"] {
            return self->Clear();
        })
    }
    pub fn context_at_index(&self, index: u32) -> SBSymbolContext {
        cpp!(unsafe [self as "SBSymbolContextList*", index as "uint32_t"] -> SBSymbolContext as "SBSymbolContext" {
            return self->GetContextAtIndex(index);
        })
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = SBSymbolContext> + 'a {
        SBIterator::new(self.len() as u32, move |index| self.context_at_index(index))
    }
}

impl IsValid for SBSymbolContextList {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBSymbolContextList*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBSymbolContextList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBSymbolContextList*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
