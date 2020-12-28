use crate::prelude::*;
use std::fs;
use std::path::{Path, PathBuf};

#[cfg(unix)]
pub fn pipe() -> Result<(fs::File, fs::File), Error> {
    use fs::File;
    use std::os::unix::prelude::*;

    unsafe {
        let mut fds = [0; 2];
        if libc::pipe(&mut fds[0] as *mut _) == 0 {
            let r = File::from_raw_fd(fds[0]);
            let w = File::from_raw_fd(fds[1]);
            Ok((r, w))
        } else {
            bail!("Failed to create a pipe.");
        }
    }
}

#[cfg(windows)]
pub fn pipe() -> Result<(fs::File, fs::File), Error> {
    use fs::File;
    use std::os::windows::prelude::*;
    use std::os::windows::raw::HANDLE;
    use std::ptr;
    use winapi::um::namedpipeapi::CreatePipe;

    unsafe {
        let mut r: HANDLE = ptr::null_mut();
        let mut w: HANDLE = ptr::null_mut();
        if CreatePipe(&mut r, &mut w, ptr::null_mut(), 4096) != 0 {
            let r = File::from_raw_handle(r);
            let w = File::from_raw_handle(w);
            Ok((r, w))
        } else {
            bail!("Failed to create a pipe.");
        }
    }
}

#[cfg(unix)]
pub fn sink() -> Result<fs::File, Error> {
    Ok(fs::File::create("/dev/null")?)
}

#[cfg(windows)]
pub fn sink() -> Result<fs::File, Error> {
    Ok(fs::File::create(r#"\\.\NUL"#)?)
}

/// Returns file path with the actual casing, as stored on disk.
#[cfg(windows)]
pub fn get_fs_path_case(path: &Path) -> Result<PathBuf, std::io::Error> {
    use std::ffi::OsString;
    use std::os::windows::ffi::{OsStrExt, OsStringExt};
    use winapi::um::fileapi::GetLongPathNameW;
    let mut wpath: Vec<u16> = path.as_os_str().encode_wide().collect();
    wpath.push(0);
    let mut buffer: Vec<u16> = Vec::with_capacity(256);
    unsafe {
        let mut size = GetLongPathNameW(wpath.as_ptr(), buffer.as_mut_ptr(), buffer.capacity() as u32) as usize;
        if size == 0 {
            return Err(std::io::Error::last_os_error());
        }
        if size > buffer.capacity() {
            buffer.reserve(size - buffer.capacity());
            size = GetLongPathNameW(wpath.as_ptr(), buffer.as_mut_ptr(), buffer.capacity() as u32) as usize;
            if size == 0 {
                return Err(std::io::Error::last_os_error());
            }
            assert!(size <= buffer.capacity());
        }
        buffer.set_len(size as usize);
    }
    Ok(PathBuf::from(OsString::from_wide(&buffer)))
}

#[cfg(unix)]
pub fn get_fs_path_case(path: &Path) -> Result<PathBuf, std::io::Error> {
    Ok(path.into())
}

#[cfg(all(unix, not(test)))]
pub fn make_case_folder() -> impl FnMut(&str) -> String {
    |k| k.into()
}

#[cfg(any(windows, test))]
pub fn make_case_folder() -> impl FnMut(&str) -> String {
    use std::collections::hash_map::Entry;
    use std::collections::HashMap;

    let mut case_map: HashMap<String, String> = HashMap::new();
    move |k| {
        let uk = k.to_uppercase();
        match case_map.entry(uk) {
            Entry::Occupied(e) => e.get().into(),
            Entry::Vacant(e) => {
                e.insert(k.into());
                k.into()
            }
        }
    }
}

#[test]
fn case_folder() {
    let mut folder = make_case_folder();
    assert_eq!(folder("Path"), "Path");
    assert_eq!(folder("PATH"), "Path");

    assert_eq!(folder("Foo"), "Foo");
    assert_eq!(folder("foo"), "Foo");
}

// #[cfg(unix)]
// pub fn waitpid(pid: u32) -> Result<(), io::Error> {
//     use std::ptr;

//     unsafe {
//         if libc::waitpid(pid as libc::pid_t, ptr::null_mut(), 0) < 0 {
//             return Err(io::Error::last_os_error()).into();
//         }
//     }
//     Ok(())
// }

// #[cfg(windows)]
// pub fn waitpid(pid: u32) -> Result<(), io::Error> {
//     use winapi::um::{
//         errhandlingapi::GetLastError, handleapi::CloseHandle, processthreadsapi::OpenProcess,
//         synchapi::WaitForSingleObject, winbase::INFINITE, winnt::PROCESS_QUERY_INFORMATION,
//     };

//     unsafe {
//         let handle = OpenProcess(PROCESS_QUERY_INFORMATION, false, pid);
//         if handle == ptr::null_mut() {
//             return Err(io::Error::last_os_error()).into();
//         }
//         WaitForSingleObject(handle, INFINITE);
//         CloseHandle(handle);
//     }
//     Ok(())
// }

// #[cfg(windows)]
// fn put_env(key: &CStr, value: &CStr) {
//     use std::os::raw::{c_char, c_int};
//     extern "C" {
//         fn _putenv_s(key: *const c_char, value: *const c_char) -> c_int;
//     }
//     unsafe {
//         _putenv_s(key.as_ptr(), value.as_ptr());
//     }
// }
