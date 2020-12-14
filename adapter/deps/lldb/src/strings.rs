use super::*;

use std::mem;

pub(crate) fn with_cstr<R, F>(s: &str, f: F) -> R
where
    F: FnOnce(*const c_char) -> R,
{
    let allocated;
    let mut buffer: [u8; 256] = unsafe { mem::uninitialized() };
    let ptr: *const c_char = if s.len() < buffer.len() {
        buffer[0..s.len()].clone_from_slice(s.as_bytes());
        buffer[s.len()] = 0;
        buffer.as_ptr() as *const c_char
    } else {
        allocated = Some(CString::new(s).unwrap());
        allocated.as_ref().unwrap().as_ptr()
    };
    f(ptr)
}

pub(crate) fn with_opt_cstr<R, F>(s: Option<&str>, f: F) -> R
where
    F: FnOnce(*const c_char) -> R,
{
    match s {
        Some(s) => with_cstr(s, f),
        None => f(ptr::null()),
    }
}

pub(crate) fn get_cstring<F>(f: F) -> CString
where
    F: Fn(*mut c_char, usize) -> usize,
{
    // Some SB API return the required size of the full string (SBThread::GetStopDescription()),
    // while others return the number of bytes actually written into the buffer (SBPath::GetPath()).
    // In the latter case we have to increase buffer capacity in a loop until the string fits.
    // There also seems to be lack of consensus on whether the terminating NUL should be included in the count or not...

    let buffer: [u8; 1024] = unsafe { mem::uninitialized() };
    let c_ptr = buffer.as_ptr() as *mut c_char;
    let size = f(c_ptr, buffer.len());
    assert!((size as isize) >= 0);
    if size < buffer.len() - 1 {
        // Must have at least 1 unused byte to be sure that we've received the whole string
        unsafe {
            return CStr::from_ptr(c_ptr).to_owned();
        }
    }

    let size_is_reliable = size > buffer.len();
    let capacity = if size_is_reliable {
        size + 1
    } else {
        buffer.len() * 2
    };
    let mut buffer = Vec::with_capacity(capacity);
    loop {
        let c_ptr = buffer.as_ptr() as *mut c_char;
        let size = f(c_ptr, buffer.capacity());
        assert!((size as isize) >= 0);
        if size < buffer.capacity() - 1 || size_is_reliable {
            assert!(size < buffer.capacity()); // should be true if size_is_reliable
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

            let iters = RefCell::new(0..2);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    store_as_cstr(&string, buff, size);
                    // Returns the required storage length
                    string.len()
                })
            );
            let iters = RefCell::new(0..2);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    store_as_cstr(&string, buff, size);
                    // Returns the required storage length, including NUL
                    string.len() + 1
                })
            );
            let iters = RefCell::new(0..100);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    // Returns stored length
                    store_as_cstr(&string, buff, size)
                })
            );
            let iters = RefCell::new(0..100);
            assert_eq!(
                cstring,
                get_cstring(|buff, size| {
                    assert!(iters.borrow_mut().next().is_some());
                    // Returns stored length, including NUL
                    store_as_cstr(&string, buff, size) + 1
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
