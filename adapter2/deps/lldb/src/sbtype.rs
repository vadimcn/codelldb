use super::*;

cpp_class!(pub unsafe struct SBType as "SBType");

unsafe impl Send for SBType {}

impl SBType {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBType*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn type_class(&self) -> TypeClass {
        cpp!(unsafe [self as "SBType*"] -> TypeClass as "TypeClass" {
            return self->GetTypeClass();
        })
    }
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBType*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn pointer_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetPointerType();
        })
    }
    pub fn pointee_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetPointeeType();
        })
    }
    pub fn reference_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetReferenceType();
        })
    }
    pub fn typedefed_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetTypedefedType();
        })
    }
    pub fn dereferenced_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetDereferencedType();
        })
    }
    pub fn unqualified_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetUnqualifiedType();
        })
    }
    pub fn array_element_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetArrayElementType();
        })
    }
    pub fn array_type(&self, size: u64) -> SBType {
        cpp!(unsafe [self as "SBType*", size as "uint64_t"] -> SBType as "SBType" {
            return self->GetArrayType(size);
        })
    }
    pub fn vector_element_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetVectorElementType();
        })
    }
    pub fn canonical_type(&self) -> SBType {
        cpp!(unsafe [self as "SBType*"] -> SBType as "SBType" {
            return self->GetCanonicalType();
        })
    }
    pub fn basic_type(&self) -> BasicType {
        cpp!(unsafe [self as "SBType*"] -> BasicType as "BasicType" {
            return self->GetBasicType();
        })
    }
    pub fn display_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBType*"] -> *const c_char as "const char*" {
            return self->GetDisplayTypeName();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
}

impl fmt::Debug for SBType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBType*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr, eDescriptionLevelFull);
            })
        })
    }
}

bitflags! {
    pub struct TypeClass : u32 {
        const Invalid = (0);
        const Array = (1 << 0);
        const BlockPointer = (1 << 1);
        const Builtin = (1 << 2);
        const Class = (1 << 3);
        const ComplexFloat = (1 << 4);
        const ComplexInteger = (1 << 5);
        const Enumeration = (1 << 6);
        const Function = (1 << 7);
        const MemberPointer = (1 << 8);
        const ObjCObject = (1 << 9);
        const ObjCInterface = (1 << 10);
        const ObjCObjectPointer = (1 << 11);
        const Pointer = (1 << 12);
        const Reference = (1 << 13);
        const Struct = (1 << 14);
        const Typedef = (1 << 15);
        const Union = (1 << 16);
        const Vector = (1 << 17);
        // Define the last type class as the MSBit of a 32 bit value
        const Other = (1 << 31);
        // Define a mask that can be used for any type when finding types
        const Any = !0;
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum BasicType {
    Invalid = 0,
    Void = 1,
    Char,
    SignedChar,
    UnsignedChar,
    WChar,
    SignedWChar,
    UnsignedWChar,
    Char16,
    Char32,
    Short,
    UnsignedShort,
    Int,
    UnsignedInt,
    Long,
    UnsignedLong,
    LongLong,
    UnsignedLongLong,
    Int128,
    UnsignedInt128,
    Bool,
    Half,
    Float,
    Double,
    LongDouble,
    FloatComplex,
    DoubleComplex,
    LongDoubleComplex,
    ObjCID,
    ObjCClass,
    ObjCSel,
    NullPtr,
    Other,
}
