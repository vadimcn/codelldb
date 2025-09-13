use super::*;
use std::path::Path;

cpp_class!(pub unsafe struct SBDebugger as "SBDebugger");

unsafe impl Send for SBDebugger {}

impl SBDebugger {
    pub const BroadcastBitProgress: u32 = (1 << 0);
    pub const BroadcastBitWarning: u32 = (1 << 1);
    pub const BroadcastBitError: u32 = (1 << 2);
    pub const BroadcastSymbolChange: u32 = (1 << 3);
    pub const BroadcastBitProgressCategory: u32 = (1 << 4);

    pub fn version_string() -> &'static str {
        let ptr = cpp!(unsafe [] ->  *const c_char as "const char*" {
            return SBDebugger::GetVersionString();
        });
        unsafe { get_str(ptr) }
    }
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
    pub fn destroy(debugger: &SBDebugger) {
        cpp!(unsafe [debugger as "SBDebugger*"] {
            SBDebugger::Destroy(*debugger);
        })
    }
    pub fn clear(&self) {
        cpp!(unsafe [self as "SBDebugger*"] {
            return self->Clear();
        })
    }
    pub fn id(&self) -> u64 {
        cpp!(unsafe [self as "SBDebugger*"] -> u64 as "uint64_t" {
            return self->GetID();
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
    pub fn set_input_file(&self, file: SBFile) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBDebugger*", file as "SBFile"] -> SBError as "SBError" {
            return self->SetInputFile(file);
        })
        .into_result()
    }
    pub fn set_output_file(&self, file: SBFile) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBDebugger*", file as "SBFile"] -> SBError as "SBError" {
            return self->SetOutputFile(file);
        })
        .into_result()
    }
    pub fn set_error_file(&self, file: SBFile) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBDebugger*", file as "SBFile"] -> SBError as "SBError" {
            return self->SetErrorFile(file);
        })
        .into_result()
    }
    pub fn create_target(
        &self,
        executable: Option<&Path>,
        target_triple: Option<&str>,
        platform_name: Option<&str>,
        add_dependent_modules: bool,
    ) -> Result<SBTarget, SBError> {
        with_opt_cstr(executable, |executable| {
            with_opt_cstr(target_triple, |target_triple| {
                with_opt_cstr(platform_name, |platform_name| {
                    let mut error = SBError::new();
                    let target = cpp!(unsafe [self as "SBDebugger*", executable as "const char*",
                                              target_triple as "const char*", platform_name as "const char*",
                                              add_dependent_modules as "bool", mut error as "SBError"
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
    pub fn dummy_target(&self) -> SBTarget {
        cpp!(unsafe [self as "SBDebugger*"] -> SBTarget as "SBTarget" {
            return self->GetDummyTarget();
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
    pub fn listener(&self) -> SBListener {
        cpp!(unsafe [self as "SBDebugger*"] -> SBListener as "SBListener" {
            return self->GetListener();
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
        unsafe { get_str(ptr) }
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
    pub fn run_command_interpreter(&self, auto_handle_events: bool, spawn_thread: bool) {
        cpp!(unsafe [self as "SBDebugger*", auto_handle_events as "bool", spawn_thread as "bool"] {
            self->RunCommandInterpreter(auto_handle_events, spawn_thread);
        })
    }
    pub fn dispatch_input(&self, input: &str) {
        let data = input.as_ptr();
        let data_len = input.len();
        cpp!(unsafe [self as "SBDebugger*", data as "const void*", data_len as "size_t"] {
            self->DispatchInput(data, data_len);
        })
    }
    pub fn dispatch_input_interrupt(&self) {
        cpp!(unsafe [self as "SBDebugger*"] {
            self->DispatchInputInterrupt();
        })
    }
    pub fn dispatch_input_end_of_file(&self) {
        cpp!(unsafe [self as "SBDebugger*"] {
            self->DispatchInputEndOfFile();
        })
    }
    pub fn broadcaster(&self) -> SBBroadcaster {
        cpp!(unsafe [self as "SBDebugger*"] -> SBBroadcaster as "SBBroadcaster" {
            return self->GetBroadcaster();
        })
    }
    pub fn broadcaster_class_name() -> &'static str {
        let ptr = cpp!(unsafe [] -> *const c_char as "const char*" {
            return SBDebugger::GetBroadcasterClass();
        });
        unsafe { get_str(ptr) }
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
