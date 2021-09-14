use crate::SBError;
use std::fs::File;
use cpp::cpp;

#[cfg(unix)]
use std::os::unix::prelude::*;
#[cfg(windows)]
use std::os::windows::prelude::*;

#[repr(C)]
pub struct FILE;

// The returned FILE takes ownership of file's descriptor.
pub fn cfile_from_file(file: File, write: bool) -> Result<*mut FILE, SBError> {
    #[cfg(unix)]
    let fd = file.into_raw_fd() as isize;
    #[cfg(windows)]
    let fd = file.into_raw_handle() as isize;

    let mut error = SBError::new();
    let cfile = cpp!(unsafe [fd as "intptr_t", write as "bool", mut error as "SBError"] -> *mut FILE as "FILE*" {
        FILE* cfile;
        #ifdef _WIN32
            cfile = fdopen(_open_osfhandle(fd, write ? 0 : _O_RDONLY), write ? "w" : "r");
        #else
            cfile = fdopen(fd, write ? "w" : "r");
        #endif
        if (cfile) {
            setvbuf(cfile, nullptr, _IOLBF, BUFSIZ);
            int x = fileno(cfile);
            if (x < 0)
                return nullptr;
            return cfile;
        } else {
            error.SetErrorToErrno();
            return nullptr;
        }
    });
    if !cfile.is_null() {
        Ok(cfile)
    } else {
        Err(error)
    }
}
