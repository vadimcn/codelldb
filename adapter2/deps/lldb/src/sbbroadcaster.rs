use super::*;

cpp_class!(pub unsafe struct SBBroadcaster as "SBBroadcaster");

unsafe impl Send for SBBroadcaster {}

impl SBBroadcaster {
    pub fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBBroadcaster*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
    pub fn add_listener(&self, listener: &SBListener, event_mask: u32) -> u32 {
        cpp!(unsafe [self as "SBBroadcaster*", listener as "SBListener*", event_mask as "uint32_t"] -> u32 as "uint32_t" {
            return self->AddListener(*listener, event_mask);
        })
    }
}
