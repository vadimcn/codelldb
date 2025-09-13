use super::*;

cpp_class!(pub unsafe struct SBStructuredData as "SBStructuredData");

unsafe impl Send for SBStructuredData {}

impl SBStructuredData {
    pub fn data_type(&self) -> StructuredDataType {
        cpp!(unsafe [self as "SBStructuredData*"] -> i32 as "int32_t" {
            return self->GetType();
        })
        .into()
    }
    pub fn value_for_key(&self, key: &str) -> SBStructuredData {
        with_cstr(key, |key| {
            cpp!(unsafe [self as "SBStructuredData*", key as "char*"] -> SBStructuredData as "SBStructuredData" {
                return self->GetValueForKey(key);
            })
        })
    }
    pub fn string_value(&self) -> String {
        get_cstring(|ptr, size| {
            cpp!(unsafe [self as "SBStructuredData*", ptr as "char*", size as "size_t"] -> usize as "size_t" {
                return self->GetStringValue(ptr, size);
            })
        })
        .into_string()
        .unwrap()
    }
    pub fn bool_value(&self, fail_value: bool) -> bool {
        cpp!(unsafe[self as "SBStructuredData*", fail_value as "bool"] -> bool as "bool" {
            return self->GetBooleanValue(fail_value);
        })
    }
    pub fn int_value(&self, fail_value: i64) -> i64 {
        cpp!(unsafe[self as "SBStructuredData*", fail_value as "int64_t"] -> i64 as "int64_t" {
            return self->GetSignedIntegerValue(fail_value);
        })
    }
    pub fn uint_value(&self, fail_value: u64) -> u64 {
        cpp!(unsafe[self as "SBStructuredData*", fail_value as "uint64_t"] -> u64 as "uint64_t" {
            return self->GetSignedIntegerValue(fail_value);
        })
    }
    pub fn float_value(&self, fail_value: f64) -> f64 {
        cpp!(unsafe[self as "SBStructuredData*", fail_value as "double"] -> f64 as "double" {
            return self->GetFloatValue(fail_value);
        })
    }
}

impl IsValid for SBStructuredData {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBStructuredData*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBStructuredData {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBStructuredData*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr).Success();
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(i32)]
pub enum StructuredDataType {
    #[default]
    Invalid = -1,
    Null = 0,
    Generic = 1,
    Array = 2,
    UnsignedInteger = 3,
    Float = 4,
    Boolean = 5,
    String = 6,
    Dictionary = 7,
    SignedInteger = 8,
}
