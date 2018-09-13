use super::*;

cpp_class!(pub unsafe struct SBAttachInfo as "SBAttachInfo");

unsafe impl Send for SBAttachInfo {}

impl SBAttachInfo {
    pub fn new() -> SBAttachInfo {
        cpp!(unsafe [] -> SBAttachInfo as "SBAttachInfo" {
            return SBAttachInfo();
        })
    }
    pub fn set_listener(&self, listener: &SBListener) {
        cpp!(unsafe [self as "SBAttachInfo*", listener as "SBListener*"] {
            self->SetListener(*listener);
        });
    }
    pub fn set_process_id(&self, pid: ProcessID) {
        cpp!(unsafe [self as "SBAttachInfo*", pid as "lldb::pid_t"] {
            self->SetProcessID(pid);
        });
    }
    pub fn process_id(&self) -> ProcessID {
        cpp!(unsafe [self as "SBAttachInfo*"] -> ProcessID as "lldb::pid_t" {
            return self->GetProcessID();
        })
    }
    pub fn set_executable(&self, path: &str) {
        with_cstr(path, |path|
            cpp!(unsafe [self as "SBAttachInfo*", path as "const char*"] {
                self->SetExecutable(path);
            })
        );
    }
    pub fn set_wait_for_launch(&self, wait_for: bool, async: bool) {
        cpp!(unsafe [self as "SBAttachInfo*", wait_for as "bool", async as "bool"] {
            self->SetWaitForLaunch(wait_for, async);
        });
    }
    pub fn wait_for_launch(&self) -> bool {
        cpp!(unsafe [self as "SBAttachInfo*"] -> bool as "bool" {
            return self->GetWaitForLaunch();
        })
    }
    pub fn set_ignore_existing(&self, ignore: bool) {
        cpp!(unsafe [self as "SBAttachInfo*", ignore as "bool"] {
            self->SetIgnoreExisting(ignore);
        });
    }
    pub fn ignore_existing(&self) -> bool {
        cpp!(unsafe [self as "SBAttachInfo*"] -> bool as "bool" {
            return self->GetIgnoreExisting();
        })
    }
    pub fn set_resume_count(&self, count: u32) {
        cpp!(unsafe [self as "SBAttachInfo*", count as "uint32_t"] {
            self->SetResumeCount(count);
        });
    }
    pub fn resume_count(&self) -> u32 {
        cpp!(unsafe [self as "SBAttachInfo*"] -> u32 as "uint32_t" {
            return self->GetResumeCount();
        })
    }
}

impl fmt::Debug for SBAttachInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Attach info: pid = {}, wait_for = {}, ignore_existing = {}, resume_count = {}", //.
            self.process_id(), self.wait_for_launch(), self.ignore_existing(), self.resume_count())
    }
}
