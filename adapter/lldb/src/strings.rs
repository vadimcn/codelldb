use std::ffi::OsStr;
use std::ffi::{CStr, CString};
use std::os::raw::c_char;
use std::ptr;
use std::str;

/// Call `f` with a NUL-terminated copy of `s`.
pub(crate) fn with_cstr<S, F, R>(s: S, f: F) -> R
where
    S: AsRef<OsStr>,
    F: FnOnce(*const c_char) -> R,
{
    #[cfg(unix)]
    fn os_str_bytes(s: &OsStr) -> &[u8] {
        use std::os::unix::prelude::OsStrExt;
        s.as_bytes()
    }
    #[cfg(windows)]
    fn os_str_bytes(s: &OsStr) -> &[u8] {
        s.to_str().unwrap().as_bytes()
    }

    let bytes = os_str_bytes(s.as_ref());
    let allocated;
    let mut buffer = [0u8; 256];
    let ptr: *const c_char = if bytes.len() < buffer.len() {
        buffer[0..bytes.len()].clone_from_slice(bytes);
        buffer[bytes.len()] = 0;
        buffer.as_ptr() as *const c_char
    } else {
        allocated = Some(CString::new(bytes).unwrap());
        allocated.as_ref().unwrap().as_ptr()
    };
    f(ptr)
}

/// Call `f` with a NUL-terminated copy of `s`, or a null pointer if `s` is None.
pub(crate) fn with_opt_cstr<S, F, R>(s: Option<S>, f: F) -> R
where
    S: AsRef<OsStr>,
    F: FnOnce(*const c_char) -> R,
{
    match s {
        Some(s) => with_cstr(s, f),
        None => f(ptr::null()),
    }
}

/// Extract CString from an API that takes pointer to a buffer and max length and
/// returns the number of bytes stored or required to stotre the entire string.
pub(crate) fn get_cstring<F>(f: F) -> CString
where
    F: Fn(*mut c_char, usize) -> usize,
{
    // Some SB API return the required size of the full string (SBThread::GetStopDescription()),
    // while others return the number of bytes actually written into the buffer (SBFileSpec::GetPath()).
    // In the latter case we have to grow buffer capacity in a loop until the string fits.
    // There also seems to be a lack of consensus whether the terminating NUL should be included in the count or not...

    let mut buffer = [0u8; 1024];
    let c_ptr = buffer.as_mut_ptr() as *mut c_char;
    let size = f(c_ptr, buffer.len());
    assert!((size as isize) >= 0);
    // Must have at least 1 unused byte to ensure that we've received the entire string.
    if size < buffer.len() - 1 {
        unsafe {
            return CStr::from_ptr(c_ptr).to_owned();
        }
    }

    let capacity = if size > buffer.len() { size + 2 } else { buffer.len() * 2 };
    let mut buffer = Vec::with_capacity(capacity);
    loop {
        let c_ptr = buffer.as_mut_ptr() as *mut c_char;
        let size = f(c_ptr, buffer.capacity());
        assert!((size as isize) >= 0);
        if size < buffer.capacity() - 1 {
            unsafe {
                let s = CStr::from_ptr(c_ptr); // Count bytes to NUL
                buffer.set_len(s.to_bytes().len());
                return CString::from_vec_unchecked(buffer);
            };
        }
        let capacity = buffer.capacity() * 2;
        buffer.reserve(capacity);
    }
}

/// Get `str` a from NUL-terminated string pointer.  If the pointer is null, returns "".
pub(crate) unsafe fn get_str<'a>(ptr: *const c_char) -> &'a str {
    if ptr.is_null() {
        ""
    } else {
        let cstr = CStr::from_ptr(ptr);
        match cstr.to_str() {
            Ok(val) => val,
            Err(err) => str::from_utf8(&cstr.to_bytes()[..err.valid_up_to()]).unwrap(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::slice;

    #[test]
    fn test_with_cstr() {
        let s = "0123456789ABCDEF";
        with_cstr(s, |c| {
            unsafe { assert_eq!(CStr::from_ptr(c).to_str().unwrap(), s) };
        });

        let s = "0123456789ABCDEF".repeat(16);
        with_cstr(&s, |c| {
            unsafe { assert_eq!(CStr::from_ptr(c).to_str().unwrap(), s) };
        });

        let s = "0123456789ABCDEF".repeat(100);
        with_cstr(&s, |c| {
            unsafe { assert_eq!(CStr::from_ptr(c).to_str().unwrap(), &s) };
        });
    }

    fn store_as_cstr(s: &str, buff: *mut c_char, size: usize) -> usize {
        let b = unsafe { slice::from_raw_parts_mut(buff as *mut u8, size) };
        let s = s.as_bytes();
        if b.len() > s.len() {
            b[..s.len()].clone_from_slice(s);
            b[s.len()] = 0;
            s.len()
        } else {
            let max = b.len() - 1;
            b[..max].clone_from_slice(&s[..max]);
            b[max] = 0;
            max
        }
    }

    #[test]
    fn test_get_cstring() {
        use std::cell::RefCell;
        for n in 0..200 {
            let string = "0123456789ABC".repeat(n);
            let cstring = CString::new(string.clone()).unwrap();

            let iters = RefCell::new(0..2); // Limit the number of iterations
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    store_as_cstr(&string, buff, size);
                    // Return the required storage length.
                    string.len()
                })
            );
            let iters = RefCell::new(0..2);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    store_as_cstr(&string, buff, size);
                    // Return the required storage length, including NUL.
                    string.len() + 1
                })
            );
            let iters = RefCell::new(0..100);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    // Return stored length.
                    store_as_cstr(&string, buff, size)
                })
            );
            let iters = RefCell::new(0..100);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    // Return stored length, including NUL.
                    store_as_cstr(&string, buff, size) + 1
                })
            );
            let iters = RefCell::new(0..100);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    // Return a value between the stored and the actual required length.
                    (store_as_cstr(&string, buff, size) + string.len()) / 2
                })
            );
        }
    }

    #[test]
    fn test_get_str() {
        assert_eq!(unsafe { get_str(b"foo\0".as_ptr() as *const c_char) }, "foo");
        assert_eq!(unsafe { get_str(b"bar\x80\0".as_ptr() as *const c_char) }, "bar");
        assert_eq!(unsafe { get_str(b"\x80\0".as_ptr() as *const c_char) }, "");
    }
}
