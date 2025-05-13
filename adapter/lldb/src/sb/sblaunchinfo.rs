use super::*;
use std::path::Path;

cpp_class!(pub unsafe struct SBLaunchInfo as "SBLaunchInfo");

unsafe impl Send for SBLaunchInfo {}

impl SBLaunchInfo {
    pub fn new() -> SBLaunchInfo {
        cpp!(unsafe [] -> SBLaunchInfo as "SBLaunchInfo" {
            return SBLaunchInfo(nullptr);
        })
    }
    pub fn clear(&mut self) {
        cpp!(unsafe [self as "SBLaunchInfo*"] {
            self->Clear();
        })
    }
    pub fn set_listener(&self, listener: &SBListener) {
        cpp!(unsafe [self as "SBLaunchInfo*", listener as "SBListener*"] {
            self->SetListener(*listener);
        })
    }
    pub fn listener(&self) -> SBListener {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> SBListener as "SBListener" {
            return self->GetListener();
        })
    }
    pub fn set_arguments<'a>(&mut self, args: impl IntoIterator<Item = &'a str>, append: bool) {
        let cstrs: Vec<CString> = args.into_iter().map(|a| CString::new(a).unwrap()).collect();
        let mut ptrs: Vec<*const c_char> = cstrs.iter().map(|cs| cs.as_ptr()).collect();
        ptrs.push(ptr::null());
        let argv = ptrs.as_ptr();
        cpp!(unsafe [self as "SBLaunchInfo*", argv as "const char**", append as "bool"] {
            self->SetArguments(argv, append);
        });
    }
    pub fn num_arguments(&self) -> u32 {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> u32 as "uint32_t" {
            return self->GetNumArguments();
        })
    }
    pub fn argument_at_index(&self, index: u32) -> &str {
        let ptr = cpp!(unsafe [self as "SBLaunchInfo*", index as "uint32_t"] -> *const c_char as "const char*" {
            return self->GetArgumentAtIndex(index);
        });
        unsafe { get_str(ptr) }
    }
    pub fn arguments<'a>(&'a self) -> impl Iterator<Item = &'a str> + 'a {
        SBIterator::new(self.num_arguments(), move |index| self.argument_at_index(index))
    }
    pub fn environment(&self) -> SBEnvironment {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> SBEnvironment as "SBEnvironment" {
            return self->GetEnvironment();
        })
    }
    pub fn set_environment(&self, env: &SBEnvironment, append: bool) {
        cpp!(unsafe [self as "SBLaunchInfo*", env as "const SBEnvironment*", append as "bool"] {
            self->SetEnvironment(*env, append);
        });
    }
    pub fn set_working_directory(&mut self, cwd: &Path) {
        with_cstr(cwd.as_os_str(), |cwd| {
            cpp!(unsafe [self as "SBLaunchInfo*", cwd as "const char*"] {
                self->SetWorkingDirectory(cwd);
            });
        })
    }
    pub fn working_directory(&self) -> Option<&Path> {
        let ptr = cpp!(unsafe [self as "SBLaunchInfo*"] -> *const c_char as "const char*" {
            return self->GetWorkingDirectory();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(Path::new(CStr::from_ptr(ptr).to_str().unwrap())) }
        }
    }
    pub fn add_open_file_action(&self, fd: i32, path: &Path, read: bool, write: bool) -> bool {
        with_cstr(path.as_os_str(), |path| {
            cpp!(unsafe [self as "SBLaunchInfo*", fd as "int32_t", path as "const char*",
                         read as "bool", write as "bool"] -> bool as "bool" {
                return self->AddOpenFileAction(fd, path, read, write);
            })
        })
    }
    pub fn add_duplicate_file_action(&self, fd: i32, dup_fd: i32) -> bool {
        cpp!(unsafe [self as "SBLaunchInfo*", fd as "int32_t", dup_fd as "int32_t"] -> bool as "bool" {
            return self->AddDuplicateFileAction(fd, dup_fd);
        })
    }
    pub fn add_suppress_file_action(&self, fd: i32, read: bool, write: bool) -> bool {
        cpp!(unsafe [self as "SBLaunchInfo*", fd as "int32_t",
                     read as "bool", write as "bool"] -> bool as "bool" {
            return self->AddSuppressFileAction(fd, read, write);
        })
    }
    pub fn add_close_file_action(&self, fd: i32) -> bool {
        cpp!(unsafe [self as "SBLaunchInfo*", fd as "int32_t"] -> bool as "bool" {
            return self->AddCloseFileAction(fd);
        })
    }
    pub fn set_launch_flags(&self, flags: LaunchFlag) {
        cpp!(unsafe [self as "SBLaunchInfo*", flags as "uint32_t"] {
            self->SetLaunchFlags(flags);
        })
    }
    pub fn launch_flags(&self) -> LaunchFlag {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> LaunchFlag as "uint32_t" {
            return self->GetLaunchFlags();
        })
    }
    pub fn set_executable_file(&self, exe_file: &SBFileSpec, add_as_first_arg: bool) {
        cpp!(unsafe [self as "SBLaunchInfo*", exe_file as "SBFileSpec*", add_as_first_arg as "bool"] {
            return self->SetExecutableFile(*exe_file, add_as_first_arg);
        })
    }
    pub fn executable_file(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> SBFileSpec as "SBFileSpec"{
            return self->GetExecutableFile();
        })
    }
    pub fn set_resume_count(&self, count: u32) {
        cpp!(unsafe [self as "SBLaunchInfo*", count as "uint32_t"] {
            self->SetResumeCount(count);
        });
    }
    pub fn resume_count(&self) -> u32 {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> u32 as "uint32_t" {
            return self->GetResumeCount();
        })
    }
    // Available after launch
    pub fn process_id(&self) -> ProcessID {
        cpp!(unsafe [self as "SBLaunchInfo*"] -> ProcessID as "lldb::pid_t" {
            return self->GetProcessID();
        })
    }
}

bitflags! {
    pub struct LaunchFlag : u32 {
        const None = 0;
        // Exec when launching and turn the calling
        // process into a new process
        const Exec = (1 << 0);
        // Stop as soon as the process launches to
        // allow the process to be debugged
        const Debug = (1 << 1);
        // Stop at the program entry point
        // instead of auto-continuing when
        // launching or attaching at entry point
        const StopAtEntry = (1 << 2);
        // Disable Address Space Layout Randomization
        const DisableASLR = (1 << 3);
        // Disable stdio for inferior process (e.g. for a GUI app)
        const DisableSTDIO = (1 << 4);
        // Launch the process in a new TTY if supported by the host
        const LaunchInTTY = (1 << 5);
        // Launch the process inside a shell to get shell expansion
        const LaunchInShell = (1 << 6);
        // Launch the process in a separate process group
        const LaunchInSeparateProcessGroup = (1 << 7);
        // If you are going to hand the process off (e.g. to debugserver)
        // set this flag so lldb & the handee don't race to set its exit status.
        const DontSetExitStatus = (1 << 8);
        // If set, then the client stub should detach rather than killing
        // the debugee if it loses connection with lldb.
        const DetachOnError = (1 << 9);
        // Perform shell-style argument expansion
        const ShellExpandArguments = (1 << 10);
        // Close the open TTY on exit
        const CloseTTYOnExit = (1 << 11);
    }
}
