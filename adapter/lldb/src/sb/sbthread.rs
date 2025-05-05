use super::*;

cpp_class!(pub unsafe struct SBThread as "SBThread");

impl SBThread {
    pub const BroadcastBitStackChanged: u32 = (1 << 0);
    pub const BroadcastBitThreadSuspended: u32 = (1 << 1);
    pub const BroadcastBitThreadResumed: u32 = (1 << 2);
    pub const BroadcastBitSelectedFrameChanged: u32 = (1 << 3);
    pub const BroadcastBitThreadSelected: u32 = (1 << 4);

    pub fn thread_id(&self) -> ThreadID {
        cpp!(unsafe [self as "SBThread*"] -> ThreadID as "tid_t" {
            return self->GetThreadID();
        })
    }
    pub fn index_id(&self) -> u32 {
        cpp!(unsafe [self as "SBThread*"] -> u32 as "uint32_t" {
            return self->GetIndexID();
        })
    }
    pub fn name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBThread*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn process(&self) -> SBProcess {
        cpp!(unsafe [self as "SBThread*"] -> SBProcess as "SBProcess" {
            return self->GetProcess();
        })
    }
    pub fn stop_reason(&self) -> StopReason {
        cpp!(unsafe [self as "SBThread*"] -> u32 as "uint32_t" {
            return self->GetStopReason();
        })
        .into()
    }
    pub fn stop_description(&self) -> String {
        get_cstring(|ptr, size| {
            cpp!(unsafe [self as "SBThread*", ptr as "char*", size as "size_t"] -> usize as "size_t" {
                return self->GetStopDescription(ptr, size);
            })
        })
        .into_string()
        .unwrap()
    }
    pub fn stop_return_value(&self) -> Option<SBValue> {
        cpp!(unsafe [self as "SBThread*"] -> SBValue as "SBValue" {
            return self->GetStopReturnValue();
        })
        .check()
    }
    pub fn stop_reason_data_count(&self) -> usize {
        cpp!(unsafe [self as "SBThread*"] -> usize as "size_t" {
            return self->GetStopReasonDataCount();
        })
    }
    /// Stop Reason              Count Data Type
    /// ======================== ===== =========================================
    /// StopReason::None          0
    /// StopReason::Trace         0
    /// StopReason::Breakpoint    N     duple: {breakpoint id, location id}
    /// StopReason::Watchpoint    1     watchpoint id
    /// StopReason::Signal        1     unix signal number
    /// StopReason::Exception     N     exception data
    /// StopReason::Exec          0
    /// StopReason::PlanComplete  0
    pub fn stop_reason_data_at_index(&self, index: usize) -> u64 {
        let index = index as u32;
        cpp!(unsafe [self as "SBThread*", index as "uint32_t"] -> u64 as "uint64_t" {
            return self->GetStopReasonDataAtIndex(index);
        })
    }
    pub fn num_frames(&self) -> u32 {
        cpp!(unsafe [self as "SBThread*"] -> u32 as "uint32_t" {
            return self->GetNumFrames();
        })
    }
    pub fn frame_at_index(&self, index: u32) -> SBFrame {
        cpp!(unsafe [self as "SBThread*", index as "uint32_t"] -> SBFrame as "SBFrame" {
            return self->GetFrameAtIndex(index);
        })
    }
    pub fn selected_frame(&self) -> SBFrame {
        cpp!(unsafe [self as "SBThread*"] -> SBFrame as "SBFrame" {
            return self->GetSelectedFrame();
        })
    }
    pub fn set_selected_frame(&self, index: u32) -> SBFrame {
        cpp!(unsafe [self as "SBThread*", index as "uint32_t"] -> SBFrame as "SBFrame" {
            return self->SetSelectedFrame(index);
        })
    }
    pub fn frames<'a>(&'a self) -> impl Iterator<Item = SBFrame> + 'a {
        SBIterator::new(self.num_frames(), move |index| self.frame_at_index(index))
    }
    pub fn resume(&self) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", mut error as "SBError"]  {
            self->Resume(error);
        });
        error.into_result()
    }
    pub fn run_to_addresss(&self, address: Address) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", address as "lldb::addr_t", mut error as "SBError"] {
            self->RunToAddress(address, error);
        });
        error.into_result()
    }
    pub fn step_over(&self, stop_others: RunMode) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", mut error as "SBError", stop_others as "lldb::RunMode"] {
            self->StepOver(stop_others, error);
        });
        error.into_result()
    }
    pub fn step_into(&self, stop_others: RunMode) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", mut error as "SBError", stop_others as "lldb::RunMode"] {
            self->StepInto(nullptr, LLDB_INVALID_LINE_NUMBER, error, stop_others);
        });
        error.into_result()
    }
    pub fn step_into_target(
        &self,
        target_name: &str,
        end_line: Option<u32>,
        stop_others: RunMode,
    ) -> Result<(), SBError> {
        with_cstr(target_name, |target_name| {
            let end_line = end_line.unwrap_or(u32::MAX);
            let mut error = SBError::new();
            cpp!(unsafe [self as "SBThread*", target_name as "const char*", end_line as "uint32_t",
                         mut error as "SBError", stop_others as "lldb::RunMode"] {
                self->StepInto(target_name, end_line, error, stop_others);
            });
            error.into_result()
        })
    }
    pub fn step_out(&self) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", mut error as "SBError"] {
            self->StepOut(error);
        });
        error.into_result()
    }
    pub fn step_instruction(&self, step_over: bool) -> Result<(), SBError> {
        let mut error = SBError::new();
        cpp!(unsafe [self as "SBThread*", step_over as "bool", mut error as "SBError"] {
            self->StepInstruction(step_over, error);
        });
        error.into_result()
    }
    pub fn jump_to_line(&self, file: &SBFileSpec, line: u32) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBThread*", file as "SBFileSpec*", line as "uint32_t"] -> SBError as "SBError" {
            return self->JumpToLine(*file, line);
        })
        .into_result()
    }
    pub fn return_from_frame(&self, frame: &SBFrame) -> Result<(), SBError> {
        cpp!(unsafe [self as "SBThread*", frame as "SBFrame*"] -> SBError as "SBError" {
            SBValue val;
            return self->ReturnFromFrame(*frame, val);
        })
        .into_result()
    }
    pub fn broadcaster_class_name() -> &'static str {
        let ptr = cpp!(unsafe [] -> *const c_char as "const char*" {
            return SBThread::GetBroadcasterClassName();
        });
        unsafe { get_str(ptr) }
    }
}

impl IsValid for SBThread {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBThread*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBThread {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBThread*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum RunMode {
    // Run only the current thread.
    OnlyThisThread = 0,
    // Run all threads.
    AllThreads = 1,
    // Run only the current thread while stepping directly through the code in the current frame,
    // but run all threads while stepping over a function call.
    OnlyDuringStepping = 2,
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(u32)]
pub enum StopReason {
    #[default]
    Invalid = 0,
    None = 1,
    Trace = 2,
    Breakpoint = 3,
    Watchpoint = 4,
    Signal = 5,
    Exception = 6,
    Exec = 7, // Program was re-exec'ed
    PlanComplete = 8,
    ThreadExiting = 9,
    Instrumentation = 10,
}
