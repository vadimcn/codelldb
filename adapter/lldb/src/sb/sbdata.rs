use super::*;
use std::marker::PhantomData;

cpp_class!(unsafe struct _SBData as "SBData");



// SBData doesn't always own the data it points to.
#[repr(transparent)]
pub struct SBData<'a> {
    _inner: _SBData,
    _marker: PhantomData<&'a ()>,
}

pub type SBDataOwned = SBData<'static>;

impl<'b> SBData<'b> {
    pub fn new() -> SBDataOwned {
        cpp!(unsafe [] -> SBData as "SBData" { return SBData(); })
    }
    pub fn borrow_bytes<'a>(bytes: &'a [u8], endian: ByteOrder, addr_size: usize) -> SBData<'a> {
        let buf = bytes.as_ptr();
        let size = bytes.len();
        let inner = cpp!(unsafe [buf as "void*", size as "size_t",
                                 endian as "ByteOrder", addr_size as "size_t"] -> _SBData as "SBData" {
            SBData data;
            SBError error; // SetData doesn't actually use the error parameter.
            data.SetData(error, buf, size, endian, addr_size);
            return data;
        });
        SBData {
            _inner: inner,
            _marker: PhantomData,
        }
    }
    pub fn from_cstr(cstr: &CStr, endian: ByteOrder, addr_size: usize) -> SBDataOwned {
        let ptr = cstr.as_ptr();
        cpp!(unsafe [ptr as "const char*", endian as "ByteOrder", addr_size as "size_t"] -> SBData as "SBData" {
            return SBData::CreateDataFromCString(endian, addr_size, ptr);
        })
    }
    pub fn clear(&mut self) {
        cpp!(unsafe [self as "SBData*"] {
            return self->Clear();
        })
    }
    pub fn byte_order(&self) -> ByteOrder {
        cpp!(unsafe [self as "SBData*"] -> ByteOrder as "ByteOrder" {
            return self->GetByteOrder();
        })
    }
    pub fn address_byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBData*"] -> usize as "size_t" {
            return (size_t)self->GetAddressByteSize();
        })
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBData*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn read_f32(&self, offset: u64) -> Result<f32, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> f32 as "float" {
            return self->GetFloat(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_f64(&self, offset: u64) -> Result<f64, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> f64 as "double" {
            return self->GetDouble(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_address(&self, offset: u64) -> Result<Address, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> Address as "addr_t" {
            return self->GetAddress(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_u8(&self, offset: u64) -> Result<u8, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> u8 as "uint8_t" {
            return self->GetUnsignedInt8(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_u16(&self, offset: u64) -> Result<u16, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> u16 as "uint16_t" {
            return self->GetUnsignedInt16(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_u32(&self, offset: u64) -> Result<u32, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> u32 as "uint32_t" {
            return self->GetUnsignedInt32(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_u64(&self, offset: u64) -> Result<u64, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> u64 as "uint64_t" {
            return self->GetUnsignedInt64(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
    }
    pub fn read_string(&self, offset: u64) -> Result<*const c_char, SBError> {
        let mut error = SBError::new();
        let result = cpp!(unsafe [self as "SBData*", mut error as "SBError", offset as "offset_t"] -> *const c_char as "const char*" {
            return self->GetString(error, offset);
        });
        if error.is_success() {
            Ok(result)
        } else {
            Err(error)
        }
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

impl<'a> IsValid for SBData<'a> {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBData*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

// TODO: impl ToOwned and Borrow

impl<'a> fmt::Debug for SBData<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBData*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ByteOrder {
    Invalid = 0,
    Big = 1,
    PDP = 2,
    Little = 4,
}
