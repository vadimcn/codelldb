use super::*;
use std::fs::File;

cpp_class!(pub unsafe struct SBDebugger as "SBDebugger");

unsafe impl Send for SBDebugger {}

impl SBDebugger {
    pub fn initialize() {
        cpp!(unsafe [] {
            SBDebugger::Initialize();
        })
    }
    pub fn terminate() {
        cpp!(unsafe [] {
            SBDebugger::Terminate();
        })
    }
    pub fn create(source_init_files: bool) -> SBDebugger {
        cpp!(unsafe [source_init_files as "bool"] -> SBDebugger as "SBDebugger" {
            return SBDebugger::Create(source_init_files);
        })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBDebugger*"] {
            return self->Clear();
        })
    }
    pub fn async_mode(&self) -> bool {
        cpp!(unsafe [self as "SBDebugger*"]-> bool as "bool" {
            return self->GetAsync();
        })
    }
    pub fn set_async_mode(&self, is_async: bool) {
        cpp!(unsafe [self as "SBDebugger*", is_async as "bool"] {
            self->SetAsync(is_async);
        })
    }
    pub fn set_input_stream(&self, file: File) -> Result<(), SBError> {
        let cfile = cfile_from_file(file, false)?;
        cpp!(unsafe [self as "SBDebugger*", cfile as "FILE*"] {
            self->SetInputFileHandle(cfile, true);
        });
        Ok(())
    }
    pub fn set_output_stream(&self, file: File) -> Result<(), SBError> {
        let cfile = cfile_from_file(file, true)?;
        cpp!(unsafe [self as "SBDebugger*", cfile as "FILE*"] {
            self->SetOutputFileHandle(cfile, true);
        });
        Ok(())
    }
    pub fn set_error_stream(&self, file: File) -> Result<(), SBError> {
        let cfile = cfile_from_file(file, true)?;
        cpp!(unsafe [self as "SBDebugger*", cfile as "FILE*"] {
            self->SetErrorFileHandle(cfile, true);
        });
        Ok(())
    }
    pub fn create_target(
        &self,
        executable: &str,
        target_triple: Option<&str>,
        platform_name: Option<&str>,
        add_dependent_modules: bool,
    ) -> Result<SBTarget, SBError> {
        with_cstr(executable, |executable| {
            with_opt_cstr(target_triple, |target_triple| {
                with_opt_cstr(platform_name, |platform_name| {
                    let mut error = SBError::new();
                    let target = cpp!(unsafe [self as "SBDebugger*", executable as "const char*", target_triple as "const char*",
                                          platform_name as "const char*", add_dependent_modules as "bool", mut error as "SBError"
                                         ] -> SBTarget as "SBTarget" {
                        return self->CreateTarget(executable, target_triple, platform_name, add_dependent_modules, error);
                    });
                    if error.is_success() {
                        Ok(target)
                    } else {
                        Err(error)
                    }
                })
            })
        })
    }
    pub fn selected_target(&self) -> SBTarget {
        cpp!(unsafe [self as "SBDebugger*"] -> SBTarget as "SBTarget" {
            return self->GetSelectedTarget();
        })
    }
    pub fn set_selected_target(&self, target: &SBTarget) {
        cpp!(unsafe [self as "SBDebugger*", target as "SBTarget*"] {
            self->SetSelectedTarget(*target);
        })
    }
    pub fn selected_platform(&self) -> SBPlatform {
        cpp!(unsafe [self as "SBDebugger*"] -> SBPlatform as "SBPlatform" {
            return self->GetSelectedPlatform();
        })
    }
    pub fn set_selected_platform(&self, platform: &SBPlatform) {
        cpp!(unsafe [self as "SBDebugger*", platform as "SBPlatform*"] {
            self->SetSelectedPlatform(*platform);
        })
    }
    pub fn command_interpreter(&self) -> SBCommandInterpreter {
        cpp!(unsafe [self as "SBDebugger*"] -> SBCommandInterpreter as "SBCommandInterpreter" {
            return self->GetCommandInterpreter();
        })
    }
    pub fn instance_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBDebugger*"] ->  *const c_char as "const char*" {
            return self->GetInstanceName();
        });
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
    }
    pub fn get_variable(&mut self, var_name: &str) -> SBStringList {
        SBDebugger::get_variable_for(self.instance_name(), var_name)
    }
    pub fn set_variable(&mut self, var_name: &str, value: &str) -> Result<(), SBError> {
        SBDebugger::set_variable_for(self.instance_name(), var_name, value)
    }
    pub fn get_variable_for(debugger_instance_name: &str, var_name: &str) -> SBStringList {
        with_cstr(debugger_instance_name, |debugger_instance_name| {
            with_cstr(var_name, |var_name| {
                cpp!(unsafe [var_name as "const char*", debugger_instance_name as "const char*"] -> SBStringList as "SBStringList" {
                    return SBDebugger::GetInternalVariableValue(var_name, debugger_instance_name);
                })
            })
        })
    }
    pub fn set_variable_for(debugger_instance_name: &str, var_name: &str, value: &str) -> Result<(), SBError> {
        with_cstr(debugger_instance_name, |debugger_instance_name| {
            with_cstr(var_name, |var_name| {
                with_cstr(value, |value| {
                    cpp!(unsafe [var_name as "const char*", value as "const char*",
                                 debugger_instance_name as "const char*"] -> SBError as "SBError" {
                        return SBDebugger::SetInternalVariable(var_name, value, debugger_instance_name);
                    })
                })
            })
        })
        .into_result()
    }
}

impl IsValid for SBDebugger {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBDebugger*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBDebugger {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBDebugger*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[cfg(unix)]
use std::os::unix::prelude::*;
#[cfg(windows)]
use std::os::windows::prelude::*;

#[repr(C)]
struct FILE;

// The returned FILE takes ownership of file's descriptor.
fn cfile_from_file(file: File, write: bool) -> Result<*mut FILE, SBError> {
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
