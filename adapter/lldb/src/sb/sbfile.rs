use super::*;

cpp_class!(pub unsafe struct SBFile as "SBFile");

unsafe impl Send for SBFile {}

impl SBFile {
    pub fn new() -> SBFile {
        cpp!(unsafe [] -> SBFile as "SBFile" { return SBFile(); })
    }
    #[cfg(unix)]
    pub fn from(obj: impl std::os::unix::io::IntoRawFd, write: bool) -> SBFile {
        SBFile::from_impl(obj.into_raw_fd(), write)
    }
    #[cfg(windows)]
    pub fn from(obj: impl std::os::windows::io::IntoRawHandle, write: bool) -> SBFile {
        let flags = if write { 0 } else { libc::O_RDONLY };
        let fd = unsafe { libc::open_osfhandle(obj.into_raw_handle() as libc::intptr_t, flags) };
        SBFile::from_impl(fd, write)
    }
    fn from_impl(fd: c_int, write: bool) -> SBFile {
        cpp!(unsafe [fd as "int", write as "bool"] -> SBFile as "SBFile" {
            return SBFile(fd, write ? "w" : "r", true);
        })
    }
    pub fn close(&self) -> SBError {
        cpp!(unsafe [self as "SBFile*"] -> SBError as "SBError" {
            return self->Close();
        })
    }
}

impl IsValid for SBFile {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBFile*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}
