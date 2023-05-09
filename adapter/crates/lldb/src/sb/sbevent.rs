use super::*;

cpp_class!(pub unsafe struct SBEvent as "SBEvent");

unsafe impl Send for SBEvent {}

impl SBEvent {
    pub fn new() -> SBEvent {
        cpp!(unsafe [] -> SBEvent as "SBEvent" {
            return SBEvent();
        })
    }
    pub fn get_cstring_from_event(event: &SBEvent) -> Option<&CStr> {
        let ptr = cpp!(unsafe [event as "SBEvent*"] -> *const c_char as "const char*" {
            return SBEvent::GetCStringFromEvent(*event);
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ptr)) }
        }
    }
    pub fn flags(&self) -> u32 {
        cpp!(unsafe [self as "SBEvent*"] -> u32 as "uint32_t" {
            return self->GetType();
        })
    }
    pub fn as_process_event(&self) -> Option<SBProcessEvent> {
        if cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return SBProcess::EventIsProcessEvent(*self);
        }) {
            Some(SBProcessEvent(self))
        } else {
            None
        }
    }
    pub fn as_breakpoint_event(&self) -> Option<SBBreakpointEvent> {
        if cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return SBBreakpoint::EventIsBreakpointEvent(*self);
        }) {
            Some(SBBreakpointEvent(self))
        } else {
            None
        }
    }
    pub fn as_watchpoint_event(&self) -> Option<SBWatchpointEvent> {
        if cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return SBWatchpoint::EventIsWatchpointEvent(*self);
        }) {
            Some(SBWatchpointEvent(self))
        } else {
            None
        }
    }
    pub fn as_target_event(&self) -> Option<SBTargetEvent> {
        if cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return SBTarget::EventIsTargetEvent(*self);
        }) {
            Some(SBTargetEvent(self))
        } else {
            None
        }
    }
    pub fn as_thread_event(&self) -> Option<SBThreadEvent> {
        if cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return SBThread::EventIsThreadEvent(*self);
        }) {
            Some(SBThreadEvent(self))
        } else {
            None
        }
    }
}

impl IsValid for SBEvent {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBEvent*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBEvent {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBEvent*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct SBProcessEvent<'a>(&'a SBEvent);

impl<'a> SBProcessEvent<'a> {
    pub fn as_event(&self) -> &SBEvent {
        self.0
    }
    pub fn process(&self) -> SBProcess {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBProcess as "SBProcess" {
            return SBProcess::GetProcessFromEvent(*event);
        })
    }
    pub fn process_state(&self) -> ProcessState {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> ProcessState as "uint32_t" {
            return SBProcess::GetStateFromEvent(*event);
        })
    }
    pub fn restarted(&self) -> bool {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> bool as "bool" {
            return SBProcess::GetRestartedFromEvent(*event);
        })
    }
    pub fn interrupted(&self) -> bool {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> bool as "bool" {
            return SBProcess::GetInterruptedFromEvent(*event);
        })
    }
    pub fn structured_data(&self) -> SBStructuredData {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBStructuredData as "SBStructuredData" {
            return SBProcess::GetStructuredDataFromEvent(*event);
        })
    }

    pub const BroadcastBitStateChanged: u32 = (1 << 0);
    pub const BroadcastBitInterrupt: u32 = (1 << 1);
    pub const BroadcastBitSTDOUT: u32 = (1 << 2);
    pub const BroadcastBitSTDERR: u32 = (1 << 3);
    pub const BroadcastBitProfileData: u32 = (1 << 4);
    pub const BroadcastBitStructuredData: u32 = (1 << 5);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct SBTargetEvent<'a>(&'a SBEvent);

impl<'a> SBTargetEvent<'a> {
    pub fn as_event(&self) -> &SBEvent {
        self.0
    }
    pub fn target(&self) -> SBTarget {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBTarget as "SBTarget" {
            return SBTarget::GetTargetFromEvent(*event);
        })
    }
    pub fn num_modules(&self) -> u32 {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> u32 as "uint32_t" {
            return SBTarget::GetNumModulesFromEvent(*event);
        })
    }
    pub fn module_at_index(&self, index: u32) -> SBModule {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*", index as "uint32_t"] -> SBModule as "SBModule" {
            return SBTarget::GetModuleAtIndexFromEvent(index, *event);
        })
    }
    pub fn modules<'b>(&'b self) -> impl Iterator<Item = SBModule> + 'b {
        SBIterator::new(self.num_modules() as u32, move |index| self.module_at_index(index))
    }

    pub const BroadcastBitBreakpointChanged: u32 = (1 << 0);
    pub const BroadcastBitModulesLoaded: u32 = (1 << 1);
    pub const BroadcastBitModulesUnloaded: u32 = (1 << 2);
    pub const BroadcastBitWatchpointChanged: u32 = (1 << 3);
    pub const BroadcastBitSymbolsLoaded: u32 = (1 << 4);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct SBThreadEvent<'a>(&'a SBEvent);

impl<'a> SBThreadEvent<'a> {
    pub fn as_event(&self) -> &SBEvent {
        self.0
    }
    pub fn thread(&self) -> SBThread {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBThread as "SBThread" {
            return SBThread::GetThreadFromEvent(*event);
        })
    }
    pub fn frame(&self) -> SBFrame {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBFrame as "SBFrame" {
            return SBThread::GetStackFrameFromEvent(*event);
        })
    }

    pub const BroadcastBitStackChanged: u32 = (1 << 0);
    pub const BroadcastBitThreadSuspended: u32 = (1 << 1);
    pub const BroadcastBitThreadResumed: u32 = (1 << 2);
    pub const BroadcastBitSelectedFrameChanged: u32 = (1 << 3);
    pub const BroadcastBitThreadSelected: u32 = (1 << 4);
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct SBBreakpointEvent<'a>(&'a SBEvent);

impl<'a> SBBreakpointEvent<'a> {
    pub fn as_event(&self) -> &SBEvent {
        self.0
    }
    pub fn breakpoint(&self) -> SBBreakpoint {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBBreakpoint as "SBBreakpoint" {
            return SBBreakpoint::GetBreakpointFromEvent(*event);
        })
    }
    pub fn event_type(&self) -> BreakpointEventType {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> BreakpointEventType as "BreakpointEventType" {
            return SBBreakpoint::GetBreakpointEventTypeFromEvent(*event);
        })
    }
    pub fn num_breakpoint_locations(&self) -> u32 {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> u32 as "uint32_t" {
            return SBBreakpoint::GetNumBreakpointLocationsFromEvent(*event);
        })
    }
    pub fn breakpoint_location_at_index(&self, index: u32) -> SBBreakpointLocation {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*", index as "uint32_t"] -> SBBreakpointLocation as "SBBreakpointLocation" {
            return SBBreakpoint::GetBreakpointLocationAtIndexFromEvent(*event, index);
        })
    }
    pub fn breakpoint_locations<'b>(&'b self) -> impl Iterator<Item = SBBreakpointLocation> + 'b {
        SBIterator::new(self.num_breakpoint_locations() as u32, move |index| {
            self.breakpoint_location_at_index(index)
        })
    }
}

bitflags! {
    pub struct BreakpointEventType : u32 {
        const InvalidType = (1 << 0);
        const Added = (1 << 1);
        const Removed = (1 << 2);
        // Locations added doesn't get sent when the breakpoint is created
        const LocationsAdded = (1 << 3);
        const LocationsRemoved = (1 << 4);
        const LocationsResolved = (1 << 5);
        const Enabled = (1 << 6);
        const Disabled = (1 << 7);
        const CommandChanged = (1 << 8);
        const ConditionChanged = (1 << 9);
        const IgnoreChanged = (1 << 10);
        const ThreadChanged = (1 << 11);
        const AutoContinueChanged = (1 << 12);
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////////////////////////

pub struct SBWatchpointEvent<'a>(&'a SBEvent);

impl<'a> SBWatchpointEvent<'a> {
    pub fn as_event(&self) -> &SBEvent {
        self.0
    }
    pub fn watchpoint(&self) -> SBWatchpoint {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> SBWatchpoint as "SBWatchpoint" {
            return SBWatchpoint::GetWatchpointFromEvent(*event);
        })
    }
    pub fn event_type(&self) -> WatchpointEventType {
        let event = self.0;
        cpp!(unsafe [event as "SBEvent*"] -> WatchpointEventType as "WatchpointEventType" {
            return SBWatchpoint::GetWatchpointEventTypeFromEvent(*event);
        })
    }
}

bitflags! {
    pub struct WatchpointEventType : u32 {
        const InvalidType = (1 << 0);
        const Added = (1 << 1);
        const Removed = (1 << 2);
        const Enabled = (1 << 6);
        const Disabled = (1 << 7);
        const CommandChanged = (1 << 8);
        const ConditionChanged = (1 << 9);
        const IgnoreChanged = (1 << 10);
        const ThreadChanged = (1 << 11);
        const TypeChanged = (1 << 12);
    }
}
