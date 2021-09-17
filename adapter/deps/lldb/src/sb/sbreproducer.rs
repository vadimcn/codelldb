use super::*;
use std::path::{Path, PathBuf};

cpp_class!(pub unsafe struct SBReproducer as "SBReproducer");

unsafe impl Send for SBReproducer {}

impl SBReproducer {
    pub fn capture(path: Option<&Path>) -> Result<(), String> {
        let error = with_opt_cstr(path, |path| {
            cpp!(unsafe [path as "const char*"] -> *const c_char as "const char*" {
                if (path != nullptr)
                    return SBReproducer::Capture(path);
                else
                    return SBReproducer::Capture();
            })
        });
        if error.is_null() {
            Ok(())
        } else {
            Err(unsafe { get_str(error) }.into())
        }
    }
    pub fn generate() -> bool {
        cpp!(unsafe [] -> bool as "bool" {
            return SBReproducer::Generate();
        })
    }
    pub fn path() -> Option<PathBuf> {
        let ptr = cpp!(unsafe [] -> *const c_char as "const char*" {
            return SBReproducer::GetPath();
        });
        if ptr.is_null() {
            None
        } else {
            let s = unsafe { get_str(ptr) };
            Some(PathBuf::from(s))
        }
    }
}
