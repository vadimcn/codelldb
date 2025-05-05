use super::*;

cpp_class!(pub unsafe struct SBUnixSignals as "SBUnixSignals");



impl SBUnixSignals {
    pub fn clear(&mut self) {
        cpp!(unsafe [self as "SBUnixSignals*"] {
            self->Clear();
        })
    }
    pub fn num_signals(&self) -> u32 {
        cpp!(unsafe [self as "SBUnixSignals*"] -> u32 as "uint32_t" {
            return self->GetNumSignals();
        })
    }
    pub fn signal_at_index(&self, index: u32) -> SignalNumber {
        cpp!(unsafe [self as "SBUnixSignals*", index as "uint32_t"] -> SignalNumber as "int" {
            return self->GetSignalAtIndex(index);
        })
    }
    pub fn signal_number_from_name(&self, name: &str) -> Option<SignalNumber> {
        let signo = with_cstr(name, |name| {
            cpp!(unsafe [self as "SBUnixSignals*", name as "const char*"] -> i32 as "int" {
                return self->GetSignalNumberFromName(name);
            })
        });
        if signo == INVALID_SIGNAL_NUMBER {
            None
        } else {
            Some(signo)
        }
    }
    pub fn signal_name(&self, signo: SignalNumber) -> Option<&CStr> {
        let ptr = cpp!(unsafe [self as "SBUnixSignals*", signo as "int"] -> *const c_char as "const char*" {
            return self->GetSignalAsCString(signo);
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(CStr::from_ptr(ptr)) }
        }
    }
    pub fn should_stop(&self, signo: SignalNumber) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int"] -> bool as "bool" {
            return self->GetShouldStop(signo);
        })
    }
    pub fn set_should_stop(&self, signo: SignalNumber, value: bool) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int", value as "bool"] -> bool as "bool" {
            return self->SetShouldStop(signo, value);
        })
    }
    pub fn should_suppress(&self, signo: SignalNumber) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int"] -> bool as "bool" {
            return self->GetShouldSuppress(signo);
        })
    }
    pub fn set_should_suppress(&self, signo: SignalNumber, value: bool) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int", value as "bool"] -> bool as "bool" {
            return self->SetShouldSuppress(signo, value);
        })
    }
    pub fn should_notify(&self, signo: SignalNumber) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int"] -> bool as "bool" {
            return self->GetShouldNotify(signo);
        })
    }
    pub fn set_should_notify(&self, signo: SignalNumber, value: bool) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*", signo as "int", value as "bool"] -> bool as "bool" {
            return self->SetShouldNotify(signo, value);
        })
    }
}

impl IsValid for SBUnixSignals {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBUnixSignals*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}
