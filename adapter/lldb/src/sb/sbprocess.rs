use super::*;

cpp_class!(pub unsafe struct SBProcess as "SBProcess");

unsafe impl Send for SBProcess {}

impl SBProcess {
    pub const BroadcastBitStateChanged: u32 = (1 << 0);
    pub const BroadcastBitInterrupt: u32 = (1 << 1);
    pub const BroadcastBitSTDOUT: u32 = (1 << 2);
    pub const BroadcastBitSTDERR: u32 = (1 << 3);
    pub const BroadcastBitProfileData: u32 = (1 << 4);
    pub const BroadcastBitStructuredData: u32 = (1 << 5);

    pub fn process_id(&self) -> ProcessID {
        cpp!(unsafe [self as "SBProcess*"] -> ProcessID as "lldb::pid_t" {
            return self->GetProcessID();
        })
    }
    pub fn target(&self) -> SBTarget {
        cpp!(unsafe [self as "SBProcess*"] -> SBTarget as "SBTarget" {
                return self->GetTarget();
        })
    }
    pub fn num_threads(&self) -> u32 {
        cpp!(unsafe [self as "SBProcess*"] -> u32 as "uint32_t" {
                return self->GetNumThreads();
        })
    }
    pub fn thread_at_index(&self, index: u32) -> SBThread {
        cpp!(unsafe [self as "SBProcess*", index as "uint32_t"] -> SBThread as "SBThread" {
            return self->GetThreadAtIndex(index);
        })
    }
    pub fn threads<'a>(&'a self) -> impl Iterator<Item = SBThread> + 'a {
        SBIterator::new(self.num_threads(), move |index| self.thread_at_index(index))
    }
    pub fn state(&self) -> ProcessState {
        cpp!(unsafe [self as "SBProcess*"] -> u32 as "uint32_t" {
            return self->GetState();
        })
        .into()
    }
    pub fn stop_id(&self, include_expression_stops: bool) -> u32 {
        cpp!(unsafe [self as "SBProcess*", include_expression_stops as "bool"] -> u32 as "uint32_t" {
            return self->GetStopID(include_expression_stops);
        })
    }
    pub fn stop_event_for_stop_id(&self, stop_id: u32) -> SBEvent {
        cpp!(unsafe [self as "SBProcess*", stop_id as "uint32_t"] -> SBEvent as "SBEvent" {
            return self->GetStopEventForStopID(stop_id);
        })
    }
    pub fn exit_status(&self) -> i32 {
        cpp!(unsafe [self as "SBProcess*"] -> i32 as "int32_t" {
            return self->GetExitStatus();
        })
    }
    pub fn selected_thread(&self) -> SBThread {
        cpp!(unsafe [self as "SBProcess*"] -> SBThread as "SBThread" {
            return self->GetSelectedThread();
        })
    }
    pub fn set_selected_thread(&self, thread: &SBThread) -> bool {
        cpp!(unsafe [self as "SBProcess*", thread as "SBThread*"] -> bool as "bool" {
            return self->SetSelectedThread(*thread);
        })
    }
    pub fn thread_by_id(&self, tid: ThreadID) -> Option<SBThread> {
        cpp!(unsafe [self as "SBProcess*", tid as "tid_t"] -> SBThread as "SBThread" {
            return self->GetThreadByID(tid);
        })
        .check()
    }
    pub fn thread_by_index_id(&self, index_id: u32) -> Option<SBThread> {
        cpp!(unsafe [self as "SBProcess*", index_id as "uint32_t"] -> SBThread as "SBThread" {
            return self->GetThreadByIndexID(index_id);
        })
        .check()
    }
    pub fn resume(&self) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBProcess*"] -> SBError as "SBError" {
            return self->Continue();
        })
        .into_result()
    }
    pub fn stop(&self) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBProcess*"] -> SBError as "SBError" {
            return self->Stop();
        })
        .into_result()
    }
    pub fn kill(&self) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBProcess*"] -> SBError as "SBError" {
            return self->Kill();
        })
        .into_result()
    }
    pub fn detach(&self, keep_stopped: bool) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBProcess*", keep_stopped as "bool"] -> SBError as "SBError" {
            return self->Detach(keep_stopped);
        })
        .into_result()
    }
    pub fn broadcaster(&self) -> SBBroadcaster {
        cpp!(unsafe [self as "SBProcess*"] -> SBBroadcaster as "SBBroadcaster" {
            return self->GetBroadcaster();
        })
    }
    pub fn broadcaster_class_name() -> &'static str {
        let ptr = cpp!(unsafe [] -> *const c_char as "const char*" {
            return SBProcess::GetBroadcasterClassName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn put_stdin(&self, buffer: &[u8]) -> usize {
        let ptr = buffer.as_ptr();
        let len = buffer.len();
        cpp!(unsafe [self as "SBProcess*", ptr as "uint8_t*", len as "size_t"] -> usize as "size_t" {
            return self->PutSTDIN((char*)ptr, len);
        })
    }
    pub fn read_stdout(&self, buffer: &mut [u8]) -> usize {
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        cpp!(unsafe [self as "SBProcess*", ptr as "uint8_t*", len as "size_t"] -> usize as "size_t" {
            return self->GetSTDOUT((char*)ptr, len);
        })
    }
    pub fn read_stderr(&self, buffer: &mut [u8]) -> usize {
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        cpp!(unsafe [self as "SBProcess*", ptr as "uint8_t*", len as "size_t"] -> usize as "size_t" {
            return self->GetSTDERR((char*)ptr, len);
        })
    }
    pub fn memory_region_info(&self, load_addr: Address) -> Result<SBMemoryRegionInfo, SBError> {
        let mut region_info = SBMemoryRegionInfo::new();
        let error = cpp!(unsafe [self as "SBProcess*", load_addr as "addr_t",
                                 mut region_info as "SBMemoryRegionInfo"] -> SBError as "SBError" {
            return self->GetMemoryRegionInfo(load_addr, region_info);
        });
        if error.is_success() {
            Ok(region_info)
        } else {
            Err(error)
        }
    }
    pub fn read_memory(&self, addr: Address, buffer: &mut [u8]) -> Result<usize, SBError> {
        let ptr = buffer.as_mut_ptr();
        let len = buffer.len();
        let mut error = SBError::new();
        let bytes_read = cpp!(unsafe [self as "SBProcess*", addr as "addr_t", ptr as "uint8_t*", len as "size_t",
                                      mut error as "SBError"] -> usize as "size_t" {
            return self->ReadMemory(addr, (void*)ptr, len, error);
        });
        if error.is_success() {
            Ok(bytes_read)
        } else {
            Err(error)
        }
    }
    pub fn write_memory(&self, addr: Address, buffer: &[u8]) -> Result<usize, SBError> {
        let ptr = buffer.as_ptr();
        let len = buffer.len();
        let mut error = SBError::new();
        let bytes_written = cpp!(unsafe [self as "SBProcess*", addr as "addr_t", ptr as "const uint8_t*", len as "size_t",
                                         mut error as "SBError"] -> usize as "size_t" {
            return self->WriteMemory(addr, (void*)ptr, len, error);
        });
        if error.is_success() {
            Ok(bytes_written)
        } else {
            Err(error)
        }
    }
    pub fn unix_signals(&self) -> SBUnixSignals {
        cpp!(unsafe [self as "SBProcess*"] -> SBUnixSignals as "SBUnixSignals" {
            return self->GetUnixSignals();
        })
    }
    pub fn signal(&self, signo: SignalNumber) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBProcess*", signo as "int"] -> SBError as "SBError" {
            return self->Signal(signo);
        })
        .into_result()
    }
}

impl IsValid for SBProcess {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBProcess*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBProcess {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBProcess*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(u32)]
pub enum ProcessState {
    #[default]
    Invalid = 0,
    /// Process is object is valid, but not currently loaded.
    Unloaded = 1,
    /// Process is connected to remote debug services, but not launched or attached to anything yet.
    Connected = 2,
    /// Process is in the process of attaching.
    Attaching = 3,
    /// Process is in the process of launching.
    Launching = 4,
    /// Process or thread is stopped and can be examined.
    Stopped = 5,
    /// Process or thread is running and can't be examined.
    Running = 6,
    /// Process or thread is in the process of stepping and can not be examined.
    Stepping = 7,
    /// Process or thread has crashed and can be examined.
    Crashed = 8,
    /// Process has been detached and can't be examined.
    Detached = 9,
    /// Process has exited and can't be examined.
    Exited = 10,
    /// Process or thread is in a suspended state as far as the debugger is concerned
    /// while other processes or threads get the chance to run.
    Suspended = 11,
}

impl ProcessState {
    /// True if the process object is backed by an actual process.
    pub fn is_alive(&self) -> bool {
        use ProcessState::*;
        match self {
            Attaching | Launching | Running | Stepping | Stopped | Suspended | Crashed => true,
            _ => false,
        }
    }
    /// True if the process is currently executing and can't be examined.
    pub fn is_running(&self) -> bool {
        use ProcessState::*;
        match self {
            Attaching | Launching | Running | Stepping => true,
            _ => false,
        }
    }
}
