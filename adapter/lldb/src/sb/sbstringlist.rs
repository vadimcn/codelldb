use super::*;

cpp_class!(pub unsafe struct SBStringList as "SBStringList");



impl SBStringList {
    pub fn new() -> SBStringList {
        cpp!(unsafe [] -> SBStringList as "SBStringList" { return SBStringList(); })
    }
    pub fn len(&self) -> usize {
        cpp!(unsafe [self as "SBStringList*"] -> usize as "size_t" {
            return self->GetSize();
        })
    }
    pub fn clear(&mut self) {
        cpp!(unsafe [self as "SBStringList*"] {
            return self->Clear();
        })
    }
    pub fn string_at_index(&self, index: u32) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBStringList*", index as "uint32_t"] -> *const c_char as "const char*" {
            return self->GetStringAtIndex(index);
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        SBIterator::new(self.len() as u32, move |index| self.string_at_index(index).unwrap())
    }
}

impl IsValid for SBStringList {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBStringList*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}
