use super::*;

cpp_class!(pub unsafe struct SBError as "SBError");

unsafe impl Send for SBError {}

impl SBError {
    pub fn new() -> SBError {
        cpp!(unsafe [] -> SBError as "SBError" { return SBError(); })
    }
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBError*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn is_success(&self) -> bool {
        cpp!(unsafe [self as "SBError*"] -> bool as "bool" {
            return self->Success();
        })
    }
    pub fn is_failure(&self) -> bool {
        cpp!(unsafe [self as "SBError*"] -> bool as "bool" {
            return self->Fail();
        })
    }
    pub fn error_string(&self) -> &str {
        let cs_ptr = cpp!(unsafe [self as "SBError*"] -> *const c_char as "const char*" {
            return self->GetCString();
        });
        match unsafe { CStr::from_ptr(cs_ptr) }.to_str() {
            Ok(s) => s,
            _ => panic!("Error strig is not valid utf8."),
        }
    }
    pub fn set_error_string(&self, string: &str) {
        with_cstr(string, |string|
            cpp!(unsafe [self as "SBError*", string as "const char*"] {
                self->SetErrorString(string);
            })
        );
    }
}

impl fmt::Debug for SBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBError*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

impl fmt::Display for SBError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(self.error_string())
    }
}

impl std::error::Error for SBError {
    fn description(&self) -> &str {
        self.error_string()
    }
}
