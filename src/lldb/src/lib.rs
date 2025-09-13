#![allow(non_upper_case_globals)]

mod strings;

mod sb;
pub use sb::*;

pub type Address = u64;
pub type ProcessID = u64;
pub type ThreadID = u64;
pub type BreakpointID = u32;
pub type WatchpointID = u32;
pub type UserID = u64;
pub type SignalNumber = i32;

pub const INVALID_ADDRESS: Address = Address::MAX;
pub const INVALID_THREAD_ID: ThreadID = 0;
pub const INVALID_PROCESS_ID: ProcessID = 0;
pub const INVALID_BREAK_ID: BreakpointID = 0;
pub const INVALID_SIGNAL_NUMBER: i32 = SignalNumber::MAX;

// Initialization for test binaries
#[cfg(test)]
#[ctor::ctor]
fn test_init() {
    use std::path::Path;
    lldb_stub::liblldb.load_from(Path::new(env!("LLDB_DYLIB"))).unwrap();
    lldb_stub::base.resolve().unwrap().mark_permanent();
    lldb_stub::v16.resolve().unwrap().mark_permanent();
}
