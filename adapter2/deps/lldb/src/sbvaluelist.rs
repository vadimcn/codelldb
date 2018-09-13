use super::*;

cpp_class!(pub unsafe struct SBValueList as "SBValueList");

unsafe impl Send for SBValueList {}

impl SBValueList {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBValueList*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn len(&self) -> usize {
        cpp!(unsafe [self as "SBValueList*"] -> usize as "size_t" {
            return self->GetSize();
        })
    }
    pub fn value_at_index(&self, index: u32) -> SBValue {
        cpp!(unsafe [self as "SBValueList*", index as "uint32_t"] -> SBValue as "SBValue" {
            return self->GetValueAtIndex(index);
        })
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = SBValue> + 'a {
        SBIterator::new(self.len() as u32, move |index| self.value_at_index(index))
    }
}
