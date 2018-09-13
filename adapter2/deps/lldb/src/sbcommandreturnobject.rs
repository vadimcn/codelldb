use super::*;

cpp_class!(pub unsafe struct SBCommandReturnObject as "SBCommandReturnObject");

unsafe impl Send for SBCommandReturnObject {}

impl SBCommandReturnObject {
    pub fn new() -> SBCommandReturnObject {
        cpp!(unsafe [] -> SBCommandReturnObject as "SBCommandReturnObject" {
            return SBCommandReturnObject();
        })
    }
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBCommandReturnObject*"] {
            return self->Clear();
        })
    }
    pub fn status(&self) -> ReturnStatus {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> ReturnStatus as "ReturnStatus" {
            return self->GetStatus();
        })
    }
    pub fn succeeded(&self) -> bool {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> bool as "bool" {
            return self->Succeeded();
        })
    }
    pub fn has_result(&self) -> bool {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> bool as "bool" {
            return self->HasResult();
        })
    }
    pub fn output_size(&self) -> usize {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> usize as "size_t" {
            return self->GetOutputSize();
        })
    }
    pub fn error_size(&self) -> usize {
        cpp!(unsafe [self as "SBCommandReturnObject*"] -> usize as "size_t" {
            return self->GetErrorSize();
        })
    }
    pub fn output(&self) -> &CStr {
        let ptr = cpp!(unsafe [self as "SBCommandReturnObject*"] -> *const c_char as "const char*" {
            return self->GetOutput();
        });
        if ptr.is_null() {
            Default::default()
        } else {
            unsafe { CStr::from_ptr(ptr) }
        }
    }
    pub fn error(&self) -> &CStr {
        let ptr = cpp!(unsafe [self as "SBCommandReturnObject*"] -> *const c_char as "const char*" {
            return self->GetError();
        });
        if ptr.is_null() {
            Default::default()
        } else {
            unsafe { CStr::from_ptr(ptr) }
        }
    }
}

impl fmt::Debug for SBCommandReturnObject {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBCommandReturnObject*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
