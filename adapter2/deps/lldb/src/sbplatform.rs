use super::*;

cpp_class!(pub unsafe struct SBPlatform as "SBPlatform");

unsafe impl Send for SBPlatform {}

impl SBPlatform {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBPlatform*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBPlatform*"] {
            return self->Clear();
        })
    }
    pub fn is_connected(&self) -> bool {
        cpp!(unsafe [self as "SBPlatform*"] -> bool as "bool" {
            return self->IsConnected();
        })
    }
    pub fn triple(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetTriple();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn hostname(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetHostname();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn os_build(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetOSBuild();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn os_description(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetOSDescription();
        });
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
}
