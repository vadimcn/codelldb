use super::*;
use std::collections::HashMap;
use std::os::raw::c_void;
use std::sync::Mutex;

cpp_class!(pub unsafe struct SBBreakpoint as "SBBreakpoint");

unsafe impl Send for SBBreakpoint {}

type BreakpointCallback = Box<dyn FnMut(&SBProcess, &SBThread, &SBBreakpointLocation) -> bool + Send>;

lazy_static::lazy_static! {
    // None means the callback is currently running.
    static ref CALLBACKS: Mutex<HashMap<SBBreakpoint, Option<BreakpointCallback>>> =
        Mutex::new(HashMap::new());
}

impl SBBreakpoint {
    pub fn id(&self) -> BreakpointID {
        cpp!(unsafe [self as "SBBreakpoint*"] -> BreakpointID as "break_id_t" {
            return self->GetID();
        })
    }
    pub fn target(&self) -> SBTarget {
        cpp!(unsafe [self as "SBBreakpoint*"] -> SBTarget as "SBTarget" {
            return self->GetTarget();
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
    // The SB API does not notify us when a breakpoint callback is discarded.
    // Clients must call clear_callback() to release the Rust closure when the breakpoint is removed.
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
            let bp = (*location).breakpoint();
            let callback = CALLBACKS.lock().unwrap().get_mut(&bp).and_then(Option::take);
            if let Some(mut callback) = callback {
                let should_stop = callback(&*process, &*thread, &*location);
                if let Some(slot) = CALLBACKS.lock().unwrap().get_mut(&bp) {
                    if slot.is_none() {
                        *slot = Some(callback);
                    }
                }
                should_stop
            } else {
                false
            }
        }

        CALLBACKS.lock().unwrap().insert(self.clone(), Some(Box::new(callback)));
        let thunk = callback_thunk as *const c_void;
        cpp!(unsafe [self as "SBBreakpoint*", thunk as "SBBreakpointHitCallback"] {
            self->SetCallback(thunk, nullptr);
        });
    }
    pub fn clear_callback(&self) {
        cpp!(unsafe [self as "SBBreakpoint*"] {
            self->SetCallback(nullptr, nullptr);
        });
        let mut callbacks = CALLBACKS.lock().unwrap();
        callbacks.remove(&self);
    }
    // Clear callbacks for which filter(&breakpoint) returns true.
    pub fn clear_callbacks(filter: impl Fn(&SBBreakpoint) -> bool) {
        let bps: Vec<SBBreakpoint> = {
            let callbacks = CALLBACKS.lock().unwrap();
            callbacks.keys().cloned().collect()
        };
        for bp in bps {
            if filter(&bp) {
                bp.clear_callback();
            }
        }
    }
}

impl IsValid for SBBreakpoint {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBBreakpoint*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl PartialEq for SBBreakpoint {
    fn eq(&self, other: &Self) -> bool {
        cpp!(unsafe [self as "SBBreakpoint*", other as "SBBreakpoint*"] -> bool as "bool" {
            return *self == *other;
        })
    }
}

impl Eq for SBBreakpoint {}

impl std::hash::Hash for SBBreakpoint {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        // Breakpoints with the same ID in different targets of one debugger have
        // identical hashes, but HashMap will distinguish them using target equality.
        self.target().debugger().id().hash(state);
        self.id().hash(state);
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
