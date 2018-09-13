use super::*;

cpp_class!(pub unsafe struct SBExecutionContext as "SBExecutionContext");

unsafe impl Send for SBExecutionContext {}

impl SBExecutionContext {
    pub fn new() -> SBExecutionContext {
        cpp!(unsafe [] -> SBExecutionContext as "SBExecutionContext" {
            return SBExecutionContext();
        })
    }
    pub fn from_target(target: &SBTarget) -> SBExecutionContext {
        cpp!(unsafe [target as "SBTarget*"] -> SBExecutionContext as "SBExecutionContext" {
            return SBExecutionContext(*target);
        })
    }
    pub fn from_process(process: &SBProcess) -> SBExecutionContext {
        cpp!(unsafe [process as "SBProcess*"] -> SBExecutionContext as "SBExecutionContext" {
            return SBExecutionContext(*process);
        })
    }
    pub fn from_thread(thread: &SBThread) -> SBExecutionContext {
        cpp!(unsafe [thread as "SBThread*"] -> SBExecutionContext as "SBExecutionContext" {
            return SBExecutionContext(*thread);
        })
    }
    pub fn from_frame(frame: &SBFrame) -> SBExecutionContext {
        cpp!(unsafe [frame as "SBFrame*"] -> SBExecutionContext as "SBExecutionContext" {
            return SBExecutionContext(*frame);
        })
    }
    pub fn frame(&self) -> Option<SBFrame> {
        let frame = cpp!(unsafe [self as "SBExecutionContext*"] -> SBFrame as "SBFrame" {
            return self->GetFrame();
        });
        if frame.is_valid() {
            Some(frame)
        } else {
            None
        }
    }
    pub fn thread(&self) -> Option<SBThread> {
        let thread = cpp!(unsafe [self as "SBExecutionContext*"] -> SBThread as "SBThread" {
            return self->GetThread();
        });
        if thread.is_valid() {
            Some(thread)
        } else {
            None
        }
    }
    pub fn process(&self) -> Option<SBProcess> {
        let process = cpp!(unsafe [self as "SBExecutionContext*"] -> SBProcess as "SBProcess" {
            return self->GetProcess();
        });
        if process.is_valid() {
            Some(process)
        } else {
            None
        }
    }
    pub fn target(&self) -> Option<SBTarget> {
        let target = cpp!(unsafe [self as "SBExecutionContext*"] -> SBTarget as "SBTarget" {
            return self->GetTarget();
        });
        if target.is_valid() {
            Some(target)
        } else {
            None
        }
    }
}
