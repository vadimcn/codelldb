use super::*;
use std::ffi::OsStr;

cpp_class!(pub unsafe struct SBEnvironment as "SBEnvironment");

unsafe impl Send for SBEnvironment {}

impl SBEnvironment {
    pub fn new() -> SBEnvironment {
        cpp!(unsafe [] -> SBEnvironment as "SBEnvironment" {
            return SBEnvironment();
        })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBEnvironment*"] {
            return self->Clear();
        })
    }
    /// Return the value of a given environment variable.
    pub fn get(&self, name: impl AsRef<OsStr>) -> Option<&CStr> {
        let ptr = with_cstr(name, |name| {
            cpp!(unsafe [self as "SBEnvironment*", name as "const char*"] -> *const c_char as "const char*" {
                return self->Get(name);
            })
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ptr)) }
        }
    }
    pub fn num_entries(&self) -> u32 {
        cpp!(unsafe [self as "SBEnvironment*"] -> u32 as "uint32_t" {
            return self->GetNumValues();
        })
    }
    pub fn entries(&self) -> SBStringList {
        cpp!(unsafe [self as "SBEnvironment*"] -> SBStringList as "SBStringList" {
            return self->GetEntries();
        })
    }
    pub fn name_at_index(&self, index: u32) -> &CStr {
        let ptr = cpp!(unsafe [self as "SBEnvironment*", index as "uint32_t"] -> *const c_char as "const char*" {
            return self->GetNameAtIndex(index);
        });
        unsafe { CStr::from_ptr(ptr) }
    }
    pub fn value_at_index(&self, index: u32) -> &CStr {
        let ptr = cpp!(unsafe [self as "SBEnvironment*", index as "uint32_t"] -> *const c_char as "const char*" {
            return self->GetValueAtIndex(index);
        });
        unsafe { CStr::from_ptr(ptr) }
    }
    /// Add or replace an existing environment variable. The input must be a string with the format
    ///     name=value
    pub fn put_entry(&self, name_and_value: impl AsRef<OsStr>) {
        with_cstr(name_and_value, |name_and_value| {
            cpp!(unsafe [self as "SBEnvironment*", name_and_value as "const char*"] {
                self->PutEntry(name_and_value);
            });
        })
    }
    /// Update this object with the given environment variables. The input is a
    /// list of entries with the same format required by SBEnvironment::PutEntry.
    ///
    /// If append is false, the provided environment will replace the existing
    /// environment. Otherwise, existing values will be updated of left untouched
    /// accordingly.
    pub fn set_entries(&self, entries: &SBStringList, append: bool) {
        cpp!(unsafe [self as "SBEnvironment*", entries as "SBStringList*", append as "bool"] {
            return self->SetEntries(*entries, append);
        })
    }
    /// Set the value of a given environment variable.
    /// If the variable exists, its value is updated only if overwrite is true.
    /// Return whether the variable was added or modified.
    pub fn set(&self, name: impl AsRef<OsStr>, value: impl AsRef<OsStr>, overwrite: bool) -> bool {
        with_cstr(name, |name| {
            with_cstr(value, |value| {
                cpp!(unsafe [self as "SBEnvironment*", name as "const char*", value as "const char*",
                             overwrite as "bool"] -> bool as "bool" {
                    return self->Set(name, value, true);
                })
            })
        })
    }
    pub fn unset(&self, name: impl AsRef<OsStr>) -> bool {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBEnvironment*", name as "const char*"] -> bool as "bool" {
                return self->Unset(name);
            })
        })
    }
}

impl fmt::Debug for SBEnvironment {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_list().entries(self.entries().iter()).finish()
    }
}
