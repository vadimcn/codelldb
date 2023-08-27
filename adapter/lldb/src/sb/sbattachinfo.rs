use super::*;
use std::path::Path;

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
    pub fn set_executable(&self, path: &Path) {
        with_cstr(path, |path| {
            cpp!(unsafe [self as "SBAttachInfo*", path as "const char*"] {
                self->SetExecutable(path);
            })
        });
    }
    /// Set attach by process name settings.
    ///
    /// Designed to be used after a call to `SBAttachInfo::set_executable()`.
    /// Future calls to `SBTarget::attach(...)` will be synchronous or
    /// asynchronous depending on the \a async argument.
    ///
    /// `wait_for`:
    ///     If `false`, attach to an existing process whose name matches.
    ///     If `true`, then wait for the next process whose name matches.
    ///
    /// `is_async`:
    ///     If `false`, then the `SBTarget::attach(...)` call will be a
    ///     synchronous call with no way to cancel the attach in
    ///     progress.
    ///     If `true`, then the `SBTarget::attach(...)` function will
    ///     return immediately and clients are expected to wait for a
    ///     process `ProcessState::Stopped` event if a suitable process is
    ///     eventually found. If the client wants to cancel the event,
    ///     SBProcess::stop() can be called and an `ProcessState::Exited` process
    ///     event will be delivered.
    pub fn set_wait_for_launch(&self, wait_for: bool, is_async: bool) {
        cpp!(unsafe [self as "SBAttachInfo*", wait_for as "bool", is_async as "bool"] {
            self->SetWaitForLaunch(wait_for, is_async);
        });
    }
    pub fn wait_for_launch(&self) -> bool {
        cpp!(unsafe [self as "SBAttachInfo*"] -> bool as "bool" {
            return self->GetWaitForLaunch();
        })
    }
    /// Ignore existing process(es).
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
        write!(
            f,
            "Attach info: pid = {}, wait_for = {}, ignore_existing = {}, resume_count = {}",
            self.process_id(),
            self.wait_for_launch(),
            self.ignore_existing(),
            self.resume_count()
        )
    }
}
