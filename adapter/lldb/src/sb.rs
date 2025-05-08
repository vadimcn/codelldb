use crate::strings::*;
use crate::*;

use std::ffi::{CStr, CString};
use std::fmt;
use std::os::raw::{c_char, c_int};
use std::ptr;
use std::slice;
use std::str;

use bitflags::bitflags;
use cpp::{cpp, cpp_class};
use num_enum::FromPrimitive;

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

cpp! {{
    #ifdef _WIN32
        #define _CRT_NONSTDC_NO_DEPRECATE 1
        #include <io.h>
        #include <fcntl.h>
    #endif
    #include <stdio.h>
    #define LLDB_API // On Windows we want the "static" symbols
    #include <lldb/API/LLDB.h>
    using namespace lldb;
}}

mod sbaddress;
mod sbattachinfo;
mod sbbreakpoint;
mod sbbreakpointlocation;
mod sbbroadcaster;
mod sbcommandinterpreter;
mod sbcommandreturnobject;
mod sbcompileunit;
mod sbdata;
mod sbdebugger;
mod sbdeclaration;
mod sberror;
mod sbevent;
mod sbexecutioncontext;
mod sbfile;
mod sbfilespec;
mod sbframe;
mod sbfunction;
mod sbinstruction;
mod sbinstructionlist;
mod sblaunchinfo;
mod sblinenetry;
mod sblistener;
mod sbmemoryregioninfo;
mod sbmodule;
mod sbmodulespec;
mod sbplatform;
mod sbprocess;
mod sbreproducer;
mod sbsection;
mod sbstream;
mod sbstringlist;
mod sbstructureddata;
mod sbsymbol;
mod sbsymbolcontext;
mod sbsymbolcontextlist;
mod sbtarget;
mod sbthread;
mod sbtype;
mod sbunixsignals;
mod sbvalue;
mod sbvaluelist;
mod sbwatchpoint;

pub use sbaddress::*;
pub use sbattachinfo::*;
pub use sbbreakpoint::*;
pub use sbbreakpointlocation::*;
pub use sbbroadcaster::*;
pub use sbcommandinterpreter::*;
pub use sbcommandreturnobject::*;
pub use sbcompileunit::*;
pub use sbdata::*;
pub use sbdebugger::*;
pub use sbdeclaration::*;
pub use sberror::*;
pub use sbevent::*;
pub use sbexecutioncontext::*;
pub use sbfile::*;
pub use sbfilespec::*;
pub use sbframe::*;
pub use sbfunction::*;
pub use sbinstruction::*;
pub use sbinstructionlist::*;
pub use sblaunchinfo::*;
pub use sblinenetry::*;
pub use sblistener::*;
pub use sbmemoryregioninfo::*;
pub use sbmodule::*;
pub use sbmodulespec::*;
pub use sbplatform::*;
pub use sbprocess::*;
pub use sbreproducer::*;
pub use sbsection::*;
pub use sbstream::*;
pub use sbstringlist::*;
pub use sbstructureddata::*;
pub use sbsymbol::*;
pub use sbsymbolcontext::*;
pub use sbsymbolcontextlist::*;
pub use sbtarget::*;
pub use sbthread::*;
pub use sbtype::*;
pub use sbunixsignals::*;
pub use sbvalue::*;
pub use sbvaluelist::*;
pub use sbwatchpoint::*;
