use super::*;

cpp_class!(pub unsafe struct SBValue as "SBValue");

unsafe impl Send for SBValue {}

impl SBValue {
    pub fn id(&self) -> UserID {
        cpp!(unsafe [self as "SBValue*"] -> UserID as "user_id_t" {
            return self->GetID();
        })
    }
    pub fn error(&self) -> SBError {
        cpp!(unsafe [self as "SBValue*"] -> SBError as "SBError" {
            return self->GetError();
        })
    }
    pub fn is_success(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            return self->GetError().Success();
        })
    }
    pub fn into_result(self) -> Result<SBValue, SBError> {
        if self.is_success() {
            Ok(self)
        } else {
            Err(self.error())
        }
    }
    pub fn type_(&self) -> SBType {
        cpp!(unsafe [self as "SBValue*"] -> SBType as "SBType" {
            return self->GetType();
        })
    }
    pub fn r#type(&self) -> SBType {
        self.type_()
    }
    pub fn name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBValue*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn type_name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBValue*"] -> *const c_char as "const char*" {
            return self->GetTypeName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn display_type_name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBValue*"] -> *const c_char as "const char*" {
            return self->GetDisplayTypeName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn address(&self) -> Option<SBAddress> {
        cpp!(unsafe [self as "SBValue*"] -> SBAddress as "SBAddress" {
            return self->GetAddress();
        })
        .check()
    }
    pub fn load_address(&self) -> Address {
        cpp!(unsafe [self as "SBValue*"] -> Address as "addr_t" {
            return self->GetLoadAddress();
        })
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBValue*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn target(&self) -> SBTarget {
        cpp!(unsafe [self as "SBValue*"] -> SBTarget as "SBTarget" {
            return self->GetTarget();
        })
    }
    pub fn process(&self) -> SBProcess {
        cpp!(unsafe [self as "SBValue*"] -> SBProcess as "SBProcess" {
            return self->GetProcess();
        })
    }
    pub fn thread(&self) -> SBThread {
        cpp!(unsafe [self as "SBValue*"] -> SBThread as "SBThread" {
            return self->GetThread();
        })
    }
    pub fn frame(&self) -> SBFrame {
        cpp!(unsafe [self as "SBValue*"] -> SBFrame as "SBFrame" {
            return self->GetFrame();
        })
    }
    pub fn is_dynamic(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            return self->IsDynamic();
        })
    }
    pub fn is_synthetic(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            return self->IsSynthetic();
        })
    }
    pub fn value_type(&self) -> ValueType {
        cpp!(unsafe [self as "SBValue*"] -> ValueType as "ValueType" {
            return self->GetValueType();
        })
    }
    pub fn value(&self) -> Option<&CStr> {
        let ptr = cpp!(unsafe [self as "SBValue*"] -> *const c_char as "const char*" {
            return self->GetValue();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ptr)) }
        }
    }
    pub fn value_as_signed(&self, fail_value: i64) -> i64 {
        cpp!(unsafe [self as "SBValue*", fail_value as "int64_t"] -> i64 as "int64_t" {
            return self->GetValueAsSigned(fail_value);
        })
    }
    pub fn value_as_unsigned(&self, fail_value: u64) -> u64 {
        cpp!(unsafe [self as "SBValue*", fail_value as "uint64_t"] -> u64 as "uint64_t" {
            return self->GetValueAsUnsigned(fail_value);
        })
    }
    pub fn try_value_as_signed(&self) -> Result<i64, SBError> {
        let mut error = SBError::new();
        let value = cpp!(unsafe [self as "SBValue*", mut error as "SBError"] -> i64 as "int64_t" {
            return self->GetValueAsSigned(error);
        });
        if error.is_success() {
            Ok(value)
        } else {
            Err(error)
        }
    }
    pub fn try_value_as_unsigned(&self) -> Result<u64, SBError> {
        let mut error = SBError::new();
        let value = cpp!(unsafe [self as "SBValue*", mut error as "SBError"] -> u64 as "uint64_t" {
            return self->GetValueAsUnsigned(error);
        });
        if error.is_success() {
            Ok(value)
        } else {
            Err(error)
        }
    }
    pub fn set_value(&self, value_str: &str) -> Result<(), SBError> {
        let mut error = SBError::new();
        let result = with_cstr(value_str, |value_str| {
            cpp!(unsafe [self as "SBValue*", value_str as "const char*", mut error as "SBError"] -> bool as "bool" {
                return self->SetValueFromCString(value_str, error);
            })
        });
        if result {
            Ok(())
        } else {
            Err(error)
        }
    }
    pub fn dereference(&self) -> SBValue {
        cpp!(unsafe [self as "SBValue*"] -> SBValue as "SBValue" {
            return self->Dereference();
        })
    }
    pub fn cast(&self, ty: &SBType) -> SBValue {
        cpp!(unsafe [self as "SBValue*", ty as "SBType*"] -> SBValue as "SBValue" {
            return self->Cast(*ty);
        })
    }
    pub fn create_child_at_offset(&self, name: &str, offset: u32, ty: &SBType) -> SBValue {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBValue*", name as "const char*",
                         offset as "uint32_t", ty as "SBType*"] ->  SBValue as "SBValue"  {
                return self->CreateChildAtOffset(name, offset, *ty);
            })
        })
    }
    pub fn data(&self) -> SBDataOwned {
        cpp!(unsafe [self as "SBValue*"] -> SBDataOwned as "SBData" {
            return self->GetData();
        })
    }
    pub fn summary(&self) -> Option<&CStr> {
        let ptr = cpp!(unsafe [self as "SBValue*"] -> *const c_char as "const char*" {
            return self->GetSummary();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ptr)) }
        }
    }
    pub fn should_hide_children(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            auto summary = self->GetTypeSummary();
            return summary.IsValid() && (summary.GetOptions() & lldb::eTypeOptionHideChildren) == lldb::eTypeOptionHideChildren;
        })
    }
    pub fn num_children(&self) -> u32 {
        cpp!(unsafe [self as "SBValue*"] -> u32 as "uint32_t" {
            return self->GetNumChildren();
        })
    }
    pub fn child_at_index(&self, index: u32) -> SBValue {
        cpp!(unsafe [self as "SBValue*", index as "uint32_t"] -> SBValue as "SBValue" {
            return self->GetChildAtIndex(index);
        })
    }
    pub fn children<'a>(&'a self) -> impl Iterator<Item = SBValue> + 'a {
        SBIterator::new(self.num_children(), move |index| self.child_at_index(index))
    }
    pub fn get_expression_path(&self, path: &mut SBStream) -> bool {
        cpp!(unsafe [self as "SBValue*", path as "SBStream*"] -> bool as "bool" {
            return self->GetExpressionPath(*path);
        })
    }
    pub fn expression_path(&self) -> Option<String> {
        let mut stm = SBStream::new();
        if self.get_expression_path(&mut stm) {
            match str::from_utf8(stm.data()) {
                Ok(s) => Some(s.to_owned()),
                Err(_) => None,
            }
        } else {
            None
        }
    }
    // Matches child members of this object and child members of any base classes.
    pub fn child_member_with_name(&self, name: &str) -> Option<SBValue> {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBValue*", name as "const char*"] ->  SBValue as "SBValue"  {
                return self->GetChildMemberWithName(name);
            })
        })
        .check()
    }
    // Matches children of this object only and will match base classes and
    // member names if this is a clang typed object.
    pub fn index_of_child_with_name(&self, name: &str) -> Option<u32> {
        let index = with_cstr(name, |name| {
            cpp!(unsafe [self as "SBValue*", name as "const char*"] ->  u32 as "uint32_t"  {
                return self->GetIndexOfChildWithName(name);
            })
        });
        if index != std::u32::MAX {
            Some(index)
        } else {
            None
        }
    }
    pub fn non_synthetic_value(&self) -> SBValue {
        cpp!(unsafe [self as "SBValue*"] ->  SBValue as "SBValue"  {
            return self->GetNonSyntheticValue();
        })
    }
    pub fn prefer_synthetic_value(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            return self->GetPreferSyntheticValue();
        })
    }
    pub fn set_prefer_synthetic_value(&self, use_synthetic: bool) {
        cpp!(unsafe [self as "SBValue*", use_synthetic as "bool"] {
            return self->SetPreferSyntheticValue(use_synthetic);
        })
    }
    pub fn format(&self) -> Format {
        cpp!(unsafe [self as "SBValue*"] -> Format as "Format" {
            return self->GetFormat();
        })
    }
    pub fn set_format(&self, format: Format) {
        cpp!(unsafe [self as "SBValue*", format as "Format"] {
            return self->SetFormat(format);
        })
    }
}

impl IsValid for SBValue {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBValue*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBValue {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBValue*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum ValueType {
    Invalid = 0,
    VariableGlobal = 1,      // globals variable
    VariableStatic = 2,      // static variable
    VariableArgument = 3,    // function argument variables
    VariableLocal = 4,       // function local variables
    Register = 5,            // stack frame register value
    RegisterSet = 6,         // A collection of stack frame register values
    ConstResult = 7,         // constant result variables
    VariableThreadLocal = 8, // thread local storage variable
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum Format {
    Default = 0,
    Boolean,
    Binary,
    Bytes,
    BytesWithASCII,
    Char,
    // Only printable characters, space if not printable
    CharPrintable,
    // Floating point complex type
    Complex,
    // NULL terminated C strings
    CString,
    Decimal,
    Enum,
    Hex,
    HexUppercase,
    Float,
    Octal,
    // OS character codes encoded into an integer 'PICT' 'text'
    // etc...
    OSType,
    Unicode16,
    Unicode32,
    Unsigned,
    Pointer,
    VectorOfChar,
    VectorOfSInt8,
    VectorOfUInt8,
    VectorOfSInt16,
    VectorOfUInt16,
    VectorOfSInt32,
    VectorOfUInt32,
    VectorOfSInt64,
    VectorOfUInt64,
    VectorOfFloat16,
    VectorOfFloat32,
    VectorOfFloat64,
    VectorOfUInt128,
    // Integer complex type
    ComplexInteger,
    // Print characters with no single quotes, used for
    // character arrays that can contain non printable
    // characters
    CharArray,
    // Describe what an address points to (func + offset with
    // file/line, symbol + offset, data, etc)
    AddressInfo,
    // ISO C99 hex float string
    HexFloat,
    // Disassemble an opcode
    Instruction,
    // Do not print this
    Void,
}

impl Format {
    pub const Invalid: Format = Format::Default;
    pub const ComplexFloat: Format = Format::Complex;
}
