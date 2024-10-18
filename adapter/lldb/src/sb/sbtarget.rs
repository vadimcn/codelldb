use super::*;
use std::path::Path;

cpp_class!(pub unsafe struct SBTarget as "SBTarget");

unsafe impl Send for SBTarget {}

impl SBTarget {
    pub fn byte_order(&self) -> ByteOrder {
        cpp!(unsafe [self as "SBTarget*"] -> ByteOrder as "ByteOrder" {
            return self->GetByteOrder();
        })
    }
    pub fn address_byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBTarget*"] -> usize as "size_t" {
            return (size_t)self->GetAddressByteSize();
        })
    }
    pub fn date_byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBTarget*"] -> usize as "size_t" {
            return (size_t)self->GetDataByteSize();
        })
    }
    pub fn code_byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBTarget*"] -> usize as "size_t" {
            return (size_t)self->GetCodeByteSize();
        })
    }
    pub fn debugger(&self) -> SBDebugger {
        cpp!(unsafe [self as "SBTarget*"] -> SBDebugger as "SBDebugger" {
            return self->GetDebugger();
        })
    }
    pub fn launch_info(&self) -> SBLaunchInfo {
        cpp!(unsafe [self as "SBTarget*"] -> SBLaunchInfo as "SBLaunchInfo" {
            return self->GetLaunchInfo();
        })
    }
    pub fn set_launch_info(&self, launch_info: &SBLaunchInfo) {
        cpp!(unsafe [self as "SBTarget*", launch_info as "const SBLaunchInfo*"]  {
            return self->SetLaunchInfo(*launch_info);
        })
    }
    pub fn executable(&self) -> SBFileSpec {
        cpp!(unsafe [self as "SBTarget*"] -> SBFileSpec as "SBFileSpec"  {
            return self->GetExecutable();
        })
    }
    pub fn add_module(&self, modulespec: &SBModuleSpec) -> SBModule {
        cpp!(unsafe [self as "SBTarget*", modulespec as "const SBModuleSpec*"] -> SBModule as "SBModule"  {
            return self->AddModule(*modulespec);
        })
    }
    pub fn launch(&self, launch_info: &SBLaunchInfo) -> Result<SBProcess, SBError> {
        let mut error = SBError::new();
        let process = cpp!(unsafe [self as "SBTarget*", launch_info as "SBLaunchInfo*", mut error as "SBError"] -> SBProcess as "SBProcess" {
            return self->Launch(*launch_info, error);
        });
        if error.is_success() {
            Ok(process)
        } else {
            Err(error)
        }
    }
    pub fn attach(&self, attach_info: &SBAttachInfo) -> Result<SBProcess, SBError> {
        let mut error = SBError::new();
        let process = cpp!(unsafe [self as "SBTarget*", attach_info as "SBAttachInfo*", mut error as "SBError"] -> SBProcess as "SBProcess" {
            return self->Attach(*attach_info, error);
        });
        if error.is_success() {
            if process.is_valid() {
                Ok(process)
            } else {
                error.set_error_string("Attach failed.");
                Err(error)
            }
        } else {
            Err(error)
        }
    }
    pub fn attach_to_process_with_id(&self, pid: ProcessID, listener: &SBListener) -> Result<SBProcess, SBError> {
        let error = SBError::new();
        let process = {
            let ref_error = &error;
            cpp!(unsafe [self as "SBTarget*", pid as "lldb::pid_t", listener as "SBListener*",
                                ref_error as "SBError*"] -> SBProcess as "SBProcess" {
                return self->AttachToProcessWithID(*listener, pid, *ref_error);
            })
        };
        if error.is_success() {
            Ok(process)
        } else {
            Err(error)
        }
    }
    pub fn process(&self) -> SBProcess {
        cpp!(unsafe [self as "SBTarget*"] -> SBProcess as "SBProcess" {
            return self->GetProcess();
        })
    }
    pub fn find_breakpoint_by_id(&self, id: BreakpointID) -> Option<SBBreakpoint> {
        cpp!(unsafe [self as "SBTarget*", id as "break_id_t"] -> SBBreakpoint as "SBBreakpoint" {
            return self->FindBreakpointByID(id);
        })
        .check()
    }
    pub fn breakpoint_create_by_location(&self, file: &Path, line: u32, column: Option<u32>) -> SBBreakpoint {
        with_cstr(file, |file| {
            let column = column.unwrap_or(0);
            cpp!(unsafe [self as "SBTarget*", file as "const char*",
                         line as "uint32_t", column as "uint32_t"] -> SBBreakpoint as "SBBreakpoint" {
                SBFileSpecList empty_list;
                return self->BreakpointCreateByLocation(file, line, column, 0, empty_list);
            })
        })
    }
    pub fn breakpoint_create_by_name(&self, name: &str) -> SBBreakpoint {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBTarget*", name as "const char*"] -> SBBreakpoint as "SBBreakpoint" {
                return self->BreakpointCreateByName(name);
            })
        })
    }
    pub fn breakpoint_create_by_regex(&self, regex: &str) -> SBBreakpoint {
        with_cstr(regex, |regex| {
            cpp!(unsafe [self as "SBTarget*", regex as "const char*"] -> SBBreakpoint as "SBBreakpoint" {
                return self->BreakpointCreateByRegex(regex);
            })
        })
    }
    pub fn breakpoint_create_for_exception(
        &self,
        language: LanguageType,
        catch_bp: bool,
        throw_bp: bool,
    ) -> SBBreakpoint {
        cpp!(unsafe [self as "SBTarget*", language as "lldb::LanguageType", catch_bp as "bool", throw_bp as "bool"] -> SBBreakpoint as "SBBreakpoint" {
            return self->BreakpointCreateForException(language, catch_bp, throw_bp);
        })
    }
    pub fn breakpoint_create_by_address(&self, address: &SBAddress) -> SBBreakpoint {
        cpp!(unsafe [self as "SBTarget*", address as "SBAddress*"] -> SBBreakpoint as "SBBreakpoint" {
            return self->BreakpointCreateBySBAddress(*address);
        })
    }
    pub fn breakpoint_create_by_load_address(&self, address: Address) -> SBBreakpoint {
        cpp!(unsafe [self as "SBTarget*", address as "addr_t"] -> SBBreakpoint as "SBBreakpoint" {
            return self->BreakpointCreateByAddress(address);
        })
    }
    pub fn breakpoint_delete(&self, id: BreakpointID) -> bool {
        cpp!(unsafe [self as "SBTarget*", id as "break_id_t"] -> bool as "bool" {
            return self->BreakpointDelete(id);
        })
    }
    pub fn watch_address(&self, addr: Address, size: usize, read: bool, write: bool) -> Result<SBWatchpoint, SBError> {
        let mut error = SBError::new();
        let wp = cpp!(unsafe [self as "SBTarget*", addr as "addr_t", size as "size_t",
                     read as "bool", write as "bool", mut error as "SBError"] -> SBWatchpoint as "SBWatchpoint" {

            //
            // The LLDB API WatchAddress is a wrapper for
            // WatchpointCreateByAddress, but it's bugged and ignores what you
            // put in 'modify', meaning it always stops even for read-only
            // breakpoing requests. Fortunately, the implementation is trivial,
            // so we can just call WatchpointCreateByAddress directly.
            //
            SBWatchpointOptions options = {};
            options.SetWatchpointTypeRead(read);
            if (write) {
              options.SetWatchpointTypeWrite(eWatchpointWriteTypeOnModify);
            }

            return self->WatchpointCreateByAddress(addr, size, options, error);
        });
        if error.is_success() {
            Ok(wp)
        } else {
            Err(error)
        }
    }
    pub fn delete_watchpoint(&self, id: WatchpointID) -> bool {
        cpp!(unsafe [self as "SBTarget*", id as "watch_id_t"] -> bool as "bool" {
            return self->DeleteWatchpoint(id);
        })
    }
    pub fn delete_all_watchpoints(&self) -> bool {
        cpp!(unsafe [self as "SBTarget*"] -> bool as "bool" {
            return self->DeleteAllWatchpoints();
        })
    }
    pub fn get_basic_type(&self, basic_type: BasicType) -> SBType {
        cpp!(unsafe [self as "SBTarget*", basic_type as "BasicType"] -> SBType as "SBType" {
            return self->GetBasicType(basic_type);
        })
    }
    pub fn create_value_from_data(&self, name: &str, data: &SBData, ty: &SBType) -> SBValue {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBTarget*", name as "const char*", data as "SBData*", ty as "SBType*"] -> SBValue as "SBValue" {
                return self->CreateValueFromData(name, *data, *ty);
            })
        })
    }
    pub fn create_value_from_address(&self, name: &str, addr: &SBAddress, ty: &SBType) -> SBValue {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBTarget*", name as "const char*", addr as "SBAddress*", ty as "SBType*"] -> SBValue as "SBValue" {
                return self->CreateValueFromAddress(name, *addr, *ty);
            })
        })
    }
    pub fn create_value_from_expression(&self, name: &str, expr: &str) -> SBValue {
        with_cstr(name, |name| {
            with_cstr(expr, |expr| {
                cpp!(unsafe [self as "SBTarget*", name as "const char*", expr as "const char*"] -> SBValue as "SBValue" {
                    return self->CreateValueFromExpression(name, expr);
                })
            })
        })
    }
    pub fn read_instructions(&self, base_addr: &SBAddress, count: u32) -> SBInstructionList {
        cpp!(unsafe [self as "SBTarget*", base_addr as "SBAddress*", count as "uint32_t"] -> SBInstructionList as "SBInstructionList" {
            SBDebugger debugger = self->GetDebugger();
            SBStringList value = SBDebugger::GetInternalVariableValue("target.x86-disassembly-flavor",
                                                                      debugger.GetInstanceName());
            const char* flavor = value.GetSize() > 0 ? value.GetStringAtIndex(0) : nullptr;
            return self->ReadInstructions(*base_addr, count, flavor);
        })
    }
    pub fn read_memory(&self, base_addr: &SBAddress, buffer: &mut [u8]) -> Result<usize, SBError> {
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        let mut error = SBError::new();
        let bytes_read = cpp!(unsafe [self as "SBTarget*", base_addr as "SBAddress*", ptr as "uint8_t*", len as "size_t",
                                      mut error as "SBError"] -> usize as "size_t" {
            return self->ReadMemory(*base_addr, (void*)ptr, len, error);
        });
        if error.is_success() {
            Ok(bytes_read)
        } else {
            Err(error)
        }
    }
    pub fn get_instructions(&self, base_addr: &SBAddress, buffer: &[u8]) -> SBInstructionList {
        let ptr = buffer.as_ptr();
        let count = buffer.len();
        cpp!(unsafe [self as "SBTarget*", base_addr as "SBAddress*",
                     ptr as "void*", count as "size_t"] -> SBInstructionList as "SBInstructionList" {
            SBDebugger debugger = self->GetDebugger();
            SBStringList value = SBDebugger::GetInternalVariableValue("target.x86-disassembly-flavor",
                                                                      debugger.GetInstanceName());
            const char* flavor = value.GetSize() > 0 ? value.GetStringAtIndex(0) : nullptr;
            return self->GetInstructionsWithFlavor(*base_addr, flavor, ptr, count);
        })
    }
    pub fn evaluate_expression(&self, expr: &str) -> SBValue {
        with_cstr(expr, |expr| {
            cpp!(unsafe [self as "SBTarget*", expr as "const char*"] -> SBValue as "SBValue" {
                return self->EvaluateExpression(expr);
            })
        })
    }
    pub fn find_functions(&self, name: &str, name_type: FunctionNameType) -> SBSymbolContextList {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBTarget*", name as "const char*", name_type as "FunctionNameType"]
                        -> SBSymbolContextList as "SBSymbolContextList" {
                return self->FindFunctions(name, name_type);
            })
        })
    }
    pub fn find_symbols(&self, name: &str, sym_type: SymbolType) -> SBSymbolContextList {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBTarget*", name as "const char*", sym_type as "SymbolType"]
                        -> SBSymbolContextList as "SBSymbolContextList" {
                return self->FindSymbols(name, sym_type);
            })
        })
    }
    pub fn resolve_symbol_context_for_address(&self, addr: &SBAddress, scope: SymbolContext) -> SBSymbolContext {
        let addr = addr as *const SBAddress;
        cpp!(unsafe [self as "SBTarget*", addr as "const SBAddress*", scope as "uint32_t"]
                    -> SBSymbolContext as "SBSymbolContext" {
            return self->ResolveSymbolContextForAddress(*addr, scope);
        })
    }
    pub fn num_modules(&self) -> u32 {
        cpp!(unsafe [self as "SBTarget*"] -> u32 as "uint32_t" {
                return self->GetNumModules();
        })
    }
    pub fn module_at_index(&self, index: u32) -> SBModule {
        cpp!(unsafe [self as "SBTarget*", index as "uint32_t"] -> SBModule as "SBModule" {
            return self->GetModuleAtIndex(index);
        })
    }
    pub fn modules<'a>(&'a self) -> impl Iterator<Item = SBModule> + 'a {
        SBIterator::new(self.num_modules(), move |index| self.module_at_index(index))
    }
    pub fn broadcaster(&self) -> SBBroadcaster {
        cpp!(unsafe [self as "SBTarget*"] -> SBBroadcaster as "SBBroadcaster" {
            return self->GetBroadcaster();
        })
    }
    pub fn broadcaster_class_name() -> &'static str {
        let ptr = cpp!(unsafe [] -> *const c_char as "const char*" {
            return SBTarget::GetBroadcasterClassName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn platform(&self) -> SBPlatform {
        cpp!(unsafe [self as "SBTarget*"] -> SBPlatform as "SBPlatform" {
            return self->GetPlatform();
        })
    }
    pub fn triple(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBTarget*"] -> *const c_char as "const char*" {
            return self->GetTriple();
        });
        unsafe { get_str(ptr) }
    }
}

impl IsValid for SBTarget {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBTarget*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBTarget {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let full = f.alternate();
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBTarget*", descr as "SBStream*", full as "bool"] -> bool as "bool" {
                return self->GetDescription(*descr, full ? eDescriptionLevelFull : eDescriptionLevelBrief);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(u32)]
#[allow(non_camel_case_types)]
pub enum LanguageType {
    #[default]
    Unknown = 0x0000,        // Unknown or invalid language value.
    C89 = 0x0001,            // ISO C:1989.
    C = 0x0002,              // Non-standardized C, such as K&R.
    Ada83 = 0x0003,          // ISO Ada:1983.
    C_plus_plus = 0x0004,    // ISO C++:1998.
    Cobol74 = 0x0005,        // ISO Cobol:1974.
    Cobol85 = 0x0006,        // ISO Cobol:1985.
    Fortran77 = 0x0007,      // ISO Fortran 77.
    Fortran90 = 0x0008,      // ISO Fortran 90.
    Pascal83 = 0x0009,       // ISO Pascal:1983.
    Modula2 = 0x000a,        // ISO Modula-2:1996.
    Java = 0x000b,           // Java.
    C99 = 0x000c,            // ISO C:1999.
    Ada95 = 0x000d,          // ISO Ada:1995.
    Fortran95 = 0x000e,      // ISO Fortran 95.
    PLI = 0x000f,            // ANSI PL/I:1976.
    ObjC = 0x0010,           // Objective-C.
    ObjC_plus_plus = 0x0011, // Objective-C++.
    UPC = 0x0012,            // Unified Parallel C.
    D = 0x0013,              // D.
    Python = 0x0014,         // Python.
    // NOTE: The below are DWARF5 constants, subject to change upon
    // completion of the DWARF5 specification
    OpenCL = 0x0015,         // OpenCL.
    Go = 0x0016,             // Go.
    Modula3 = 0x0017,        // Modula 3.
    Haskell = 0x0018,        // Haskell.
    C_plus_plus_03 = 0x0019, // ISO C++:2003.
    C_plus_plus_11 = 0x001a, // ISO C++:2011.
    OCaml = 0x001b,          // OCaml.
    Rust = 0x001c,           // Rust.
    C11 = 0x001d,            // ISO C:2011.
    Swift = 0x001e,          // Swift.
    Julia = 0x001f,          // Julia.
    Dylan = 0x0020,          // Dylan.
    C_plus_plus_14 = 0x0021, // ISO C++:2014.
    Fortran03 = 0x0022,      // ISO Fortran 2003.
    Fortran08 = 0x0023,      // ISO Fortran 2008.
    // Vendor Extensions
    // Note: Language::GetNameForLanguageType
    // assumes these can be used as indexes into array language_names, and
    // Language::SetLanguageFromCString and Language::AsCString
    // assume these can be used as indexes into array g_languages.
    MipsAssembler = 0x0024,   // Mips_Assembler.
    ExtRenderScript = 0x0025, // RenderScript.
}

bitflags! {
    pub struct FunctionNameType : u32 {
        const None = 0;
        // Automatically figure out which FunctionNameType
        // bits to set based on the function name.
        const Auto = (1 << 1);
        // The function name.
        // For C this is the same as just the name of the function
        // For C++ this is the mangled or demangled version of the mangled name.
        // For ObjC this is the full function signature with the + or
        // - and the square brackets and the class and selector
        const Full = (1<< 2);
        // The function name only, no namespaces
        // or arguments and no class
        // methods or selectors will be searched.
        const Base = (1 << 3);
        // Find function by method name (C++)
        // with no namespace or arguments
        const Method = (1 << 4);
        // Find function by selector name (ObjC) names
        const Selector = (1 << 5);
    }
}

// These mask bits allow a common interface for queries that can
// limit the amount of information that gets parsed to only the
// information that is requested. These bits also can indicate what
// actually did get resolved during query function calls.
//
// Each definition corresponds to a one of the member variables
// in this class, and requests that that item be resolved, or
// indicates that the member did get resolved.
bitflags! {
    pub struct SymbolContext : u32 {
        ///< Set when \a target is requested from
        ///a query, or was located in query
        ///results
        const Target = (1 << 0);
        ///< Set when \a module is requested from
        ///a query, or was located in query
        ///results
        const Module = (1 << 1);
        ///< Set when \a comp_unit is requested
        ///from a query, or was located in query
        ///results
        const CompUnit = (1 << 2);
        ///< Set when \a function is requested
        ///from a query, or was located in query
        ///results
        const Function = (1 << 3);
        ///< Set when the deepest \a block is
        ///requested from a query, or was located
        ///in query results
        const Block = (1 << 4);
        ///< Set when \a line_entry is
        ///requested from a query, or was
        ///located in query results
        const LineEntry = (1 << 5);
        ///< Set when \a symbol is requested from
        ///a query, or was located in query
        ///results
        const Symbol = (1 << 6);
        ///< Indicates to try and lookup everything
        ///up during a routine symbol context
        ///query.
        const Everything = ((1 << 7) - 1);
        ///< Set when \a global or static
        ///variable is requested from a query, or
        ///was located in query results.
        ///< Variable is potentially expensive to lookup so it isn't
        ///included in
        ///< Everything which stops it from being used during frame PC
        ///lookups and
        ///< many other potential address to symbol context lookups.
        const Variable = (1 << 7);
    }
}
