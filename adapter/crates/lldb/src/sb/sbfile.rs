use super::*;

cpp_class!(pub unsafe struct SBFile as "SBFile");

unsafe impl Send for SBFile {}

#[repr(C)]
pub struct FILE;

impl SBFile {
    pub fn new() -> SBFile {
        cpp!(unsafe [] -> SBFile as "SBFile" { return SBFile(); })
    }
    pub fn from(fd: impl std::os::unix::io::IntoRawFd, mode: &str) -> SBFile {
        let fd = fd.into_raw_fd();
        with_cstr(mode, |mode| {
            cpp!(unsafe [fd as "int", mode as "const char*"] -> SBFile as "SBFile" {
                return SBFile(fd, mode, true);
            })
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
