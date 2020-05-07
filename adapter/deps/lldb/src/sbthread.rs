use super::*;

cpp_class!(pub unsafe struct SBThread as "SBThread");

unsafe impl Send for SBThread {}

impl SBThread {
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
            unsafe { Some(CStr::from_ptr(ptr).to_str().unwrap()) }
        }
    }
    pub fn process(&self) -> SBProcess {
        cpp!(unsafe [self as "SBThread*"] -> SBProcess as "SBProcess" {
            return self->GetProcess();
        })
    }
    pub fn stop_reason(&self) -> StopReason {
        cpp!(unsafe [self as "SBThread*"] -> StopReason as "uint32_t" {
            return self->GetStopReason();
        })
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
    pub fn step_over(&self, stop_others: RunMode) {
        cpp!(unsafe [self as "SBThread*", stop_others as "lldb::RunMode"] {
            self->StepOver(stop_others);
        })
    }
    pub fn step_into(&self, stop_others: RunMode) {
        cpp!(unsafe [self as "SBThread*", stop_others as "lldb::RunMode"] {
            self->StepInto(stop_others);
        })
    }
    pub fn step_out(&self) {
        cpp!(unsafe [self as "SBThread*"] {
            self->StepOut();
        })
    }
    pub fn step_instruction(&self, step_over: bool) {
        cpp!(unsafe [self as "SBThread*", step_over as "bool"] {
            self->StepInstruction(step_over);
        })
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
        assert!(!ptr.is_null());
        unsafe { CStr::from_ptr(ptr).to_str().unwrap() }
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

#[derive(Clone, Copy, Eq, PartialEq, Debug)]
#[repr(u32)]
pub enum StopReason {
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
