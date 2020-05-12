use super::*;
use std::path::Path;

cpp_class!(pub unsafe struct SBPlatform as "SBPlatform");

unsafe impl Send for SBPlatform {}

impl SBPlatform {
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBPlatform*"] {
            return self->Clear();
        })
    }
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
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
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn hostname(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetHostname();
        });
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn os_build(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetOSBuild();
        });
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn os_description(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBPlatform*"] -> *const c_char as "const char*" {
            return self->GetOSDescription();
        });
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn get_file_permissions(&self, path: &Path) -> u32 {
        with_cstr(path.to_str().unwrap(), |path| {
            cpp!(unsafe [self as "SBPlatform*", path as "const char*"] -> u32 as "uint32_t" {
                return self->GetFilePermissions(path);
            })
        })
    }
    pub fn launch(&self, launch_info: &SBLaunchInfo) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBPlatform*", launch_info as "SBLaunchInfo*"] -> SBError as "SBError" {
            return self->Launch(*launch_info);
        })
        .into_result()
    }
    pub fn kill(&self, pid: ProcessID) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBPlatform*", pid as "lldb::pid_t"] -> SBError as "SBError" {
            return self->Kill(pid);
        })
        .into_result()
    }
}

impl IsValid for SBPlatform {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBPlatform*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}
