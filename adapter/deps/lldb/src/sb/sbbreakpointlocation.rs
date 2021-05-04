use super::*;

cpp_class!(pub unsafe struct SBBreakpointLocation as "SBBreakpointLocation");

unsafe impl Send for SBBreakpointLocation {}

impl SBBreakpointLocation {
    pub fn id(&self) -> BreakpointID {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> BreakpointID as "break_id_t" {
            return self->GetID();
        })
    }
    pub fn address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> SBAddress as "SBAddress" {
            return self->GetAddress();
        })
    }
    pub fn breakpoint(&self) -> SBBreakpoint {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> SBBreakpoint as "SBBreakpoint" {
            return self->GetBreakpoint();
        })
    }
    pub fn is_enabled(&self) -> bool {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> bool as "bool" {
            return self->IsEnabled();
        })
    }
    pub fn set_enabled(&self, enabled: bool) {
        cpp!(unsafe [self as "SBBreakpointLocation*", enabled as "bool"] {
            self->SetEnabled(enabled);
        })
    }
    pub fn is_resolved(&self) -> bool {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> bool as "bool" {
            return self->IsResolved();
        })
    }
}

impl IsValid for SBBreakpointLocation {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBBreakpointLocation*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBBreakpointLocation {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBBreakpointLocation*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr, eDescriptionLevelFull);
            })
        })
    }
}
