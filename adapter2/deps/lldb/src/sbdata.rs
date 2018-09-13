use super::*;

cpp_class!(pub unsafe struct SBData as "SBData");

unsafe impl Send for SBData {}

impl SBData {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBData*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBData*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn read_raw_data(&self, offset: u64, buffer: &mut [u8]) -> Result<(), SBError> {
        let ptr = buffer.as_ptr();
        let size = buffer.len();
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t",
                     ptr as "void*", size as "size_t"] -> usize as "size_t" {
            return self->ReadRawData(error, offset, ptr, size);
        });
        if error.is_success() {
            Ok(())
        } else {
            Err(error)
        }
    }
}

impl fmt::Debug for SBData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBData*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
