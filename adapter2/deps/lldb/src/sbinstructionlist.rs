use super::*;

cpp_class!(pub unsafe struct SBInstructionList as "SBInstructionList");

unsafe impl Send for SBInstructionList {}

impl SBInstructionList {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBInstructionList*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn len(&self) -> usize {
        cpp!(unsafe [self as "SBInstructionList*"] -> usize as "size_t" {
            return self->GetSize();
        })
    }
    pub fn instruction_at_index(&self, index: u32) -> SBInstruction {
        cpp!(unsafe [self as "SBInstructionList*", index as "uint32_t"] -> SBInstruction as "SBInstruction" {
            return self->GetInstructionAtIndex(index);
        })
    }
    pub fn iter<'a>(&'a self) -> impl Iterator<Item = SBInstruction> + 'a {
        SBIterator::new(self.len() as u32, move |index| self.instruction_at_index(index))
    }
}

impl fmt::Debug for SBInstructionList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBInstructionList*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
