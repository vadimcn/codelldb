use super::*;

cpp_class!(pub unsafe struct SBBroadcaster as "SBBroadcaster");

impl SBBroadcaster {
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBBroadcaster*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn add_listener(&self, listener: &SBListener, event_mask: u32) -> u32 {
        cpp!(unsafe [self as "SBBroadcaster*", listener as "SBListener*", event_mask as "uint32_t"] -> u32 as "uint32_t" {
            return self->AddListener(*listener, event_mask);
        })
    }
}

impl IsValid for SBBroadcaster {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBBroadcaster*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}
