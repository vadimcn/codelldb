pub use platform::*;
pub type Handle = *const std::os::raw::c_void;

#[cfg(unix)]
mod platform {
    use failure::*;
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_void};
    use std::path::Path;

    pub const DYLIB_PREFIX: &str = "lib";
    #[cfg(target_os = "linux")]
    pub const DYLIB_EXTENSION: &str = "so";
    #[cfg(target_os = "macos")]
    pub const DYLIB_EXTENSION: &str = "dylib";
    pub const DYLIB_SUBDIR: &str = "lib";

    #[link(name = "dl")]
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> *const c_void;
        fn dlsym(handle: *const c_void, symbol: *const c_char) -> *const c_void;
        fn dlerror() -> *const c_char;
    }
    const RTLD_LAZY: c_int = 0x00001;
    const RTLD_GLOBAL: c_int = 0x00100;

    pub unsafe fn load_library(path: &Path, global_symbols: bool) -> Result<*const c_void, Error> {
        let cpath = CString::new(path.as_os_str().to_str().unwrap().as_bytes()).unwrap();
        let flags = match global_symbols {
            true => RTLD_LAZY | RTLD_GLOBAL,
            false => RTLD_LAZY,
        };
        let handle = dlopen(cpath.as_ptr() as *const c_char, flags);
        if handle.is_null() {
            Err(format_err!("{:?}", CStr::from_ptr(dlerror())))
        } else {
            Ok(handle)
        }
    }

    pub unsafe fn find_symbol(handle: *const c_void, name: &str) -> Result<*const c_void, Error> {
        let cname = CString::new(name).unwrap();
        let ptr = dlsym(handle, cname.as_ptr() as *const c_char);
        if ptr.is_null() {
            Err(format_err!("{:?}", CStr::from_ptr(dlerror())))
        } else {
            Ok(ptr)
        }
    }
}

#[cfg(windows)]
mod platform {
    use failure::*;
    use std::ffi::CString;
    use std::os::raw::{c_char, c_void};
    use std::path::Path;

    pub const DYLIB_PREFIX: &str = "";
    pub const DYLIB_EXTENSION: &str = "dll";
    pub const DYLIB_SUBDIR: &str = "bin";

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryA(filename: *const c_char) -> *const c_void;
        fn GetProcAddress(handle: *const c_void, symbol: *const c_char) -> *const c_void;
        fn GetLastError() -> u32;
    }

    pub unsafe fn load_library(path: &Path, _global_symbols: bool) -> Result<*const c_void, Error> {
        let cpath = CString::new(path.as_os_str().to_str().unwrap().as_bytes()).unwrap();
        let handle = LoadLibraryA(cpath.as_ptr() as *const c_char);
        if handle.is_null() {
            Err(format!("Could not load {:?} (err={:08X})", path, GetLastError()).into())
        } else {
            Ok(handle)
        }
    }

    pub unsafe fn find_symbol(handle: *const c_void, name: &str) -> Result<*const c_void, Error> {
        let cname = CString::new(name).unwrap();
        let ptr = GetProcAddress(handle, cname.as_ptr() as *const c_char);
        if ptr.is_null() {
            Err(format!("Could not find {} (err={:08X})", name, GetLastError()).into())
        } else {
            Ok(ptr)
        }
    }
}
