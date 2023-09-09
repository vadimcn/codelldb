pub use platform::*;
pub type Handle = *const std::os::raw::c_void;
pub type Error = Box<dyn std::error::Error>;

pub const CURRENT_PROCESS: Handle = 0 as Handle;

#[cfg(unix)]
mod platform {
    use super::{Error, Handle, CURRENT_PROCESS};
    use std::ffi::{CStr, CString};
    use std::os::raw::{c_char, c_int, c_void};
    use std::path::Path;

    pub const DYLIB_SUBDIR: &str = "lib";
    pub const DYLIB_PREFIX: &str = "lib";
    #[cfg(any(target_os = "linux", target_os = "freebsd"))]
    pub const DYLIB_EXTENSION: &str = "so";
    #[cfg(target_os = "macos")]
    pub const DYLIB_EXTENSION: &str = "dylib";

    #[cfg(target_os = "linux")]
    mod constants {
        use super::*;
        pub const RTLD_LAZY: c_int = 0x1;
        pub const RTLD_GLOBAL: c_int = 0x100;
        pub const RTLD_DEFAULT: Handle = 0 as Handle;
    }
    #[cfg(target_os = "freebsd")]
    mod constants {
        // https://github.com/freebsd/freebsd-src/blob/b2dcde7e9e75dd35124d12e68036e56ee650d568/include/dlfcn.h#L42
        use super::*;
        pub const RTLD_LAZY: c_int = 0x1;
        pub const RTLD_GLOBAL: c_int = 0x100;
        pub const RTLD_DEFAULT: Handle = -2isize as Handle;
    }
    #[cfg(target_os = "macos")]
    mod constants {
        use super::*;
        pub const RTLD_LAZY: c_int = 0x1;
        pub const RTLD_GLOBAL: c_int = 0x8;
        pub const RTLD_DEFAULT: Handle = -2i32 as Handle;
    }
    pub use constants::*;

    #[link(name = "dl")]
    extern "C" {
        fn dlopen(filename: *const c_char, flag: c_int) -> Handle;
        fn dlclose(handle: Handle) -> c_int;
        fn dlsym(handle: Handle, symbol: *const c_char) -> *const c_void;
        fn dlerror() -> *const c_char;
    }

    pub unsafe fn load_library(path: &Path, global_symbols: bool) -> Result<Handle, Error> {
        let cpath = CString::new(path.as_os_str().to_str().unwrap().as_bytes()).unwrap();
        let flags = match global_symbols {
            true => RTLD_LAZY | RTLD_GLOBAL,
            false => RTLD_LAZY,
        };
        let handle = dlopen(cpath.as_ptr() as *const c_char, flags);
        if handle.is_null() {
            Err(format!("{:?}", CStr::from_ptr(dlerror())).into())
        } else {
            Ok(handle)
        }
    }

    pub unsafe fn free_library(handle: Handle) -> Result<(), Error> {
        if dlclose(handle) == 0 {
            Ok(())
        } else {
            Err(format!("{:?}", CStr::from_ptr(dlerror())).into())
        }
    }

    pub unsafe fn find_symbol(handle: Handle, name: &str) -> Result<*const c_void, Error> {
        let cname = CString::new(name).unwrap();
        let handle = match handle {
            CURRENT_PROCESS => RTLD_DEFAULT,
            _ => handle,
        };
        let ptr = dlsym(handle, cname.as_ptr() as *const c_char);
        if ptr.is_null() {
            Err(format!("{:?}", CStr::from_ptr(dlerror())).into())
        } else {
            Ok(ptr)
        }
    }
}

#[cfg(windows)]
mod platform {
    use super::{Error, Handle};
    use std::ffi::{CString, OsStr, OsString};
    use std::os::raw::{c_char, c_void};
    use std::os::windows::ffi::*;
    use std::path::Path;
    use std::{env, ptr};

    pub const DYLIB_PREFIX: &str = "";
    pub const DYLIB_EXTENSION: &str = "dll";
    pub const DYLIB_SUBDIR: &str = "bin";

    const LOAD_WITH_ALTERED_SEARCH_PATH: u32 = 0x0008;

    #[link(name = "kernel32")]
    extern "system" {
        fn LoadLibraryExW(filename: *const u16, hfile: Handle, flags: u32) -> Handle;
        fn FreeLibrary(handle: Handle) -> u32;
        fn GetProcAddress(handle: Handle, symbol: *const c_char) -> *const c_void;
        fn GetLastError() -> u32;
    }

    fn to_wstr(s: &OsStr) -> Vec<u16> {
        s.encode_wide().chain(Some(0)).collect::<Vec<_>>()
    }

    pub fn add_library_directory(path: &Path) -> Result<(), Error> {
        if !path.is_dir() {
            return Err("Not a directory".into());
        }
        let mut os_path = OsString::from(path);
        if let Some(val) = env::var_os("PATH") {
            os_path.push(";");
            os_path.push(val);
        }
        env::set_var("PATH", &os_path);
        Ok(())
    }

    pub unsafe fn load_library(path: &Path, _global_symbols: bool) -> Result<Handle, Error> {
        let flags = if path.is_absolute() { LOAD_WITH_ALTERED_SEARCH_PATH } else { 0 };
        let handle = LoadLibraryExW(to_wstr(path.as_os_str()).as_ptr(), ptr::null(), flags);
        if handle.is_null() {
            Err(format!("Could not load {:?} (err={:08X})", path, GetLastError()).into())
        } else {
            Ok(handle)
        }
    }

    pub unsafe fn free_library(handle: Handle) -> Result<(), Error> {
        if FreeLibrary(handle) != 0 {
            Ok(())
        } else {
            Err(format!("Could not free library (err={:08X})", GetLastError()).into())
        }
    }

    pub unsafe fn find_symbol(handle: Handle, name: &str) -> Result<*const c_void, Error> {
        let cname = CString::new(name).unwrap();
        let ptr = GetProcAddress(handle, cname.as_ptr() as *const c_char);
        if ptr.is_null() {
            Err(format!("Could not find {} (err={:08X})", name, GetLastError()).into())
        } else {
            Ok(ptr)
        }
    }
}
