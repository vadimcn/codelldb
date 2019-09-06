use super::*;

cpp_class!(pub unsafe struct SBWatchpoint as "SBWatchpoint");

unsafe impl Send for SBWatchpoint {}

impl SBWatchpoint {
    pub fn id(&self) -> WatchpointID {
        cpp!(unsafe [self as "SBWatchpoint*"] -> WatchpointID as "watch_id_t" {
            return self->GetID();
        })
    }
}

impl IsValid for SBWatchpoint {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBWatchpoint*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBWatchpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let full = f.alternate();
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBWatchpoint*", descr as "SBStream*", full as "bool"] -> bool as "bool" {
                return self->GetDescription(*descr, full ? eDescriptionLevelFull : eDescriptionLevelBrief);
            })
        })
    }
}
