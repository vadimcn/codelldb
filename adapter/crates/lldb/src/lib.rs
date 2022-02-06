#![allow(non_upper_case_globals)]
#![allow(deprecated)] // TODO: check for rust-cpp updates

use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::{c_char, c_int, c_uint};
use std::ptr;
use std::slice;
use std::str;

pub type Address = u64;
pub type ProcessID = u64;
pub type ThreadID = u64;
pub type BreakpointID = u32;
pub type WatchpointID = u32;
pub type UserID = u64;

pub const INVALID_ADDRESS: Address = Address::max_value();
pub const INVALID_THREAD_ID: ThreadID = 0;
pub const INVALID_PROCESS_ID: ProcessID = 0;
pub const INVALID_BREAK_ID: BreakpointID = 0;

/////////////////////////////////////////////////////////////////////////////////////////////////////

fn debug_descr<CPP>(f: &mut fmt::Formatter, cpp: CPP) -> fmt::Result
where
    CPP: FnOnce(&mut SBStream) -> bool,
{
    let mut descr = SBStream::new();
    if cpp(&mut descr) {
        match str::from_utf8(descr.data()) {
            Ok(s) => f.write_str(s),
            Err(_) => Err(fmt::Error),
        }
    } else {
        Ok(())
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

struct SBIterator<Item, GetItem>
where
    GetItem: FnMut(u32) -> Item,
{
    size: u32,
    get_item: GetItem,
    index: u32,
}

impl<Item, GetItem> SBIterator<Item, GetItem>
where
    GetItem: FnMut(u32) -> Item,
{
    fn new(size: u32, get_item: GetItem) -> Self {
        Self {
            size: size,
            get_item: get_item,
            index: 0,
        }
    }
}

impl<Item, GetItem> Iterator for SBIterator<Item, GetItem>
where
    GetItem: FnMut(u32) -> Item,
{
    type Item = Item;
    fn next(&mut self) -> Option<Self::Item> {
        if self.index < self.size {
            self.index += 1;
            Some((self.get_item)(self.index - 1))
        } else {
            None
        }
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        return (0, Some(self.size as usize));
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

pub trait IsValid {
    fn is_valid(&self) -> bool;

    /// If `self.is_valid()` is `true`, returns `Some(self)`, otherwise `None`.
    fn check(self) -> Option<Self>
    where
        Self: Sized,
    {
        if self.is_valid() {
            Some(self)
        } else {
            None
        }
    }
}

/////////////////////////////////////////////////////////////////////////////////////////////////////

mod cfile;
mod strings;

mod sb {
    use super::strings::*;
    use super::*;
    use bitflags::bitflags;
    use cpp::{cpp, cpp_class};

    cpp! {{
        #ifdef _WIN32
            #define _CRT_NONSTDC_NO_DEPRECATE 1
            #include <io.h>
            #include <fcntl.h>
        #endif
        #include <stdio.h>
        #include <lldb/API/LLDB.h>
        using namespace lldb;
    }}

    pub mod sbaddress;
    pub mod sbattachinfo;
    pub mod sbbreakpoint;
    pub mod sbbreakpointlocation;
    pub mod sbbroadcaster;
    pub mod sbcommandinterpreter;
    pub mod sbcommandreturnobject;
    pub mod sbcompileunit;
    pub mod sbdata;
    pub mod sbdebugger;
    pub mod sberror;
    pub mod sbevent;
    pub mod sbexecutioncontext;
    pub mod sbfilespec;
    pub mod sbframe;
    pub mod sbinstruction;
    pub mod sbinstructionlist;
    pub mod sblaunchinfo;
    pub mod sblinenetry;
    pub mod sblistener;
    pub mod sbmemoryregioninfo;
    pub mod sbmodule;
    pub mod sbmodulespec;
    pub mod sbplatform;
    pub mod sbprocess;
    pub mod sbreproducer;
    pub mod sbstream;
    pub mod sbstringlist;
    pub mod sbsymbol;
    pub mod sbsymbolcontext;
    pub mod sbsymbolcontextlist;
    pub mod sbtarget;
    pub mod sbthread;
    pub mod sbtype;
    pub mod sbvalue;
    pub mod sbvaluelist;
    pub mod sbwatchpoint;
}

pub use sb::sbaddress::*;
pub use sb::sbattachinfo::*;
pub use sb::sbbreakpoint::*;
pub use sb::sbbreakpointlocation::*;
pub use sb::sbbroadcaster::*;
pub use sb::sbcommandinterpreter::*;
pub use sb::sbcommandreturnobject::*;
pub use sb::sbcompileunit::*;
pub use sb::sbdata::*;
pub use sb::sbdebugger::*;
pub use sb::sberror::*;
pub use sb::sbevent::*;
pub use sb::sbexecutioncontext::*;
pub use sb::sbfilespec::*;
pub use sb::sbframe::*;
pub use sb::sbinstruction::*;
pub use sb::sbinstructionlist::*;
pub use sb::sblaunchinfo::*;
pub use sb::sblinenetry::*;
pub use sb::sblistener::*;
pub use sb::sbmemoryregioninfo::*;
pub use sb::sbmodule::*;
pub use sb::sbmodulespec::*;
pub use sb::sbplatform::*;
pub use sb::sbprocess::*;
pub use sb::sbreproducer::*;
pub use sb::sbstream::*;
pub use sb::sbstringlist::*;
pub use sb::sbsymbol::*;
pub use sb::sbsymbolcontext::*;
pub use sb::sbsymbolcontextlist::*;
pub use sb::sbtarget::*;
pub use sb::sbthread::*;
pub use sb::sbtype::*;
pub use sb::sbvalue::*;
pub use sb::sbvaluelist::*;
pub use sb::sbwatchpoint::*;
