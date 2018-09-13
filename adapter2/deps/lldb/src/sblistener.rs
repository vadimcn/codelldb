use super::*;

cpp_class!(pub unsafe struct SBListener as "SBListener");

unsafe impl Send for SBListener {}

impl SBListener {
    pub fn new() -> SBListener {
        cpp!(unsafe [] -> SBListener as "SBListener" {
            return SBListener();
        })
    }
    pub fn new_with_name(name: &str) -> SBListener {
        with_cstr(name, |name| {
            cpp!(unsafe [name as "const char*"] -> SBListener as "SBListener" {
                return SBListener(name);
            })
        })
    }
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBListener*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn wait_for_event(&self, num_seconds: u32, event: &mut SBEvent) -> bool {
        cpp!(unsafe [self as "SBListener*", num_seconds as "uint32_t", event as "SBEvent*"] -> bool as "bool" {
            return self->WaitForEvent(num_seconds, *event);
        })
    }
    pub fn peek_at_next_event(&self, event: &mut SBEvent) -> bool {
        cpp!(unsafe [self as "SBListener*", event as "SBEvent*"] -> bool as "bool" {
            return self->PeekAtNextEvent(*event);
        })
    }
    // returns effective event mask
    pub fn start_listening_for_event_class(&self, debugger: &SBDebugger, broadcaster_class: &str, event_mask: u32) -> u32 {
        with_cstr(broadcaster_class, |broadcaster_class| {
            cpp!(unsafe [self as "SBListener*", debugger as "SBDebugger*",
                         broadcaster_class as "const char*", event_mask as "uint32_t"] -> u32 as "uint32_t" {
                return self->StartListeningForEventClass(*debugger, broadcaster_class, event_mask);
            })
        })
    }
    pub fn stop_listening_for_event_class(&self, debugger: &SBDebugger, broadcaster_class: &str, event_mask: u32) -> bool {
        with_cstr(broadcaster_class, |broadcaster_class| {
            cpp!(unsafe [self as "SBListener*", debugger as "SBDebugger*",
                         broadcaster_class as "const char*", event_mask as "uint32_t"] -> bool as "bool" {
                return self->StopListeningForEventClass(*debugger, broadcaster_class, event_mask);
            })
        })
    }
}
