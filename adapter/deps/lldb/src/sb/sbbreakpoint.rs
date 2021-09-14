use super::*;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::Mutex;

cpp_class!(pub unsafe struct SBBreakpoint as "SBBreakpoint");

unsafe impl Send for SBBreakpoint {}

lazy_static::lazy_static! {
    static ref CALLBACKS: Mutex<HashMap<BreakpointID, Box<dyn FnMut(&SBProcess, &SBThread, &SBBreakpointLocation) -> bool + Send>>> =
        Mutex::new(HashMap::new());
}

impl SBBreakpoint {
    pub fn id(&self) -> BreakpointID {
        cpp!(unsafe [self as "SBBreakpoint*"] -> BreakpointID as "break_id_t" {
            return self->GetID();
        })
    }
    /// How many locations have been matched in debug info.
    pub fn num_locations(&self) -> u32 {
        cpp!(unsafe [self as "SBBreakpoint*"] -> usize as "size_t" {
            return self->GetNumLocations();
        }) as u32
    }
    /// How many locations have been mapped to physical address in modules loaded by the current process.
    pub fn num_resolved_locations(&self) -> u32 {
        cpp!(unsafe [self as "SBBreakpoint*"] -> usize as "size_t" {
            return self->GetNumResolvedLocations();
        }) as u32
    }
    pub fn location_at_index(&self, index: u32) -> SBBreakpointLocation {
        cpp!(unsafe [self as "SBBreakpoint*", index as "uint32_t"] -> SBBreakpointLocation as "SBBreakpointLocation" {
            return self->GetLocationAtIndex(index);
        })
    }
    pub fn locations<'a>(&'a self) -> impl Iterator<Item = SBBreakpointLocation> + 'a {
        SBIterator::new(self.num_locations(), move |index| self.location_at_index(index))
    }
    pub fn condition(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBBreakpoint*"] -> *const c_char as "const char*" {
            return self->GetCondition();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn set_condition(&self, condition: &str) {
        with_cstr(condition, |condition| {
            cpp!(unsafe [self as "SBBreakpoint*", condition as "const char*"] {
                self->SetCondition(condition);
            });
        });
    }
    pub fn hit_count(&self) -> u32 {
        cpp!(unsafe [self as "SBBreakpoint*"] -> u32 as "uint32_t" {
            return self->GetHitCount();
        })
    }
    pub fn add_name(&self, name: &str) -> bool {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBBreakpoint*", name as "const char*"] -> bool as "bool" {
                return self->AddName(name);
            })
        })
    }
    pub fn remove_name(&self, name: &str) {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBBreakpoint*", name as "const char*"] {
                self->RemoveName(name);
            })
        })
    }
    // LLDB API does not provide automatic tracking of callback lifetimes; in order to prevent a memory leak,
    // be sure to call clear_callback().  The easiest way to accomplish this is by watching for "breakpoint removed"
    // events.
    pub fn set_callback<F>(&self, callback: F)
    where
        F: FnMut(&SBProcess, &SBThread, &SBBreakpointLocation) -> bool + Send + 'static,
    {
        unsafe extern "C" fn callback_thunk(
            _baton: *mut c_void,
            process: *const SBProcess,
            thread: *const SBThread,
            location: *const SBBreakpointLocation,
        ) -> bool {
            let bp_id = (*location).breakpoint().id();
            let mut callbacks = CALLBACKS.lock().unwrap();
            if let Some(callback) = callbacks.get_mut(&bp_id) {
                callback(&*process, &*thread, &*location)
            } else {
                false
            }
        }

        let bp_id = self.id();
        let mut callbacks = CALLBACKS.lock().unwrap();
        callbacks.insert(bp_id, Box::new(callback));

        let cb = callback_thunk as *const c_void;
        cpp!(unsafe [self as "SBBreakpoint*", cb as "SBBreakpointHitCallback"] {
            self->SetCallback(cb, nullptr);
        });
    }
    pub fn clear_callback(&self) {
        cpp!(unsafe [self as "SBBreakpoint*"] {
            self->SetCallback(nullptr, nullptr);
        });
        let mut callbacks = CALLBACKS.lock().unwrap();
        callbacks.remove(&self.id());
    }
    pub fn clear_all_callbacks() {
        let mut callbacks = CALLBACKS.lock().unwrap();
        callbacks.clear();
    }
}

impl IsValid for SBBreakpoint {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBBreakpoint*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBBreakpoint {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let with_locations = f.alternate();
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBBreakpoint*", with_locations as "bool", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr, with_locations);
            })
        })
    }
}
