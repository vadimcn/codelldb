use super::*;
use std::path::{Path, PathBuf};

cpp_class!(pub unsafe struct SBFileSpec as "SBFileSpec");

unsafe impl Send for SBFileSpec {}

impl SBFileSpec {
    pub fn filename(&self) -> &Path {
        unsafe { get_str(self.filename_ptr()).as_ref() }
    }
    pub fn directory(&self) -> &Path {
        unsafe { get_str(self.directory_ptr()).as_ref() }
    }
    pub fn path(&self) -> PathBuf {
        get_cstring(|ptr, size| {
            cpp!(unsafe [self as "SBFileSpec*", ptr as "char*", size as "size_t"] -> u32 as "uint32_t" {
                return self->GetPath(ptr, size);
            }) as usize
        })
        .into_string()
        .unwrap()
        .into()
    }
    // If pattern contains directory path, then do a full compare, otherwise just compare filenames.
    pub fn matches(&self, pattern: &SBFileSpec) -> bool {
        let ptr = pattern.directory_ptr();
        if ptr.is_null() || unsafe { *ptr == 0 } {
            self.filename() == pattern.filename()
        } else {
            self == pattern
        }
    }
    fn directory_ptr(&self) -> *const c_char {
        cpp!(unsafe [self as "SBFileSpec*"] -> *const c_char as "const char*" {
            return self->GetDirectory();
        })
    }
    fn filename_ptr(&self) -> *const c_char {
        cpp!(unsafe [self as "SBFileSpec*"] -> *const c_char as "const char*" {
            return self->GetFilename();
        })
    }
}

impl IsValid for SBFileSpec {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBFileSpec*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl PartialEq for SBFileSpec {
    fn eq(&self, other: &Self) -> bool {
        cpp!(unsafe [self as "SBFileSpec*", other as "SBFileSpec*"] -> bool as "bool" {
            return *self == *other;
        })
    }
}

impl<T> From<T> for SBFileSpec
where
    T: AsRef<Path>,
{
    fn from(path: T) -> Self {
        with_cstr(path.as_ref().to_str().unwrap(), |path| {
            cpp!(unsafe [path as "const char*"] -> SBFileSpec as "SBFileSpec" {
                return SBFileSpec(path);
            })
        })
    }
}

impl fmt::Debug for SBFileSpec {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBFileSpec*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
