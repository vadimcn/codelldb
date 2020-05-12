#![allow(dead_code)]
use crate::error::Error;
use std::fs;
use std::io;

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
