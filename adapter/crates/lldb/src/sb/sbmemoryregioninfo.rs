use super::*;

cpp_class!(pub unsafe struct SBMemoryRegionInfo as "SBMemoryRegionInfo");

unsafe impl Send for SBMemoryRegionInfo {}

impl SBMemoryRegionInfo {
    pub fn new() -> SBMemoryRegionInfo {
        cpp!(unsafe [] -> SBMemoryRegionInfo as "SBMemoryRegionInfo" {
            return SBMemoryRegionInfo();
        })
    }
    pub fn region_base(&self) -> Address {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> Address as "addr_t" {
            return self->GetRegionBase();
        })
    }
    pub fn region_end(&self) -> Address {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> Address as "addr_t" {
            return self->GetRegionEnd();
        })
    }
    pub fn is_readable(&self) -> bool {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> bool as "bool" {
            return self->IsReadable();
        })
    }
    pub fn is_writable(&self) -> bool {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> bool as "bool" {
            return self->IsWritable();
        })
    }
    pub fn is_executable(&self) -> bool {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> bool as "bool" {
            return self->IsExecutable();
        })
    }
    pub fn is_mapped(&self) -> bool {
        cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> bool as "bool" {
            return self->IsMapped();
        })
    }
    pub fn name(&self) -> Option<&'static str> {
        let ptr = cpp!(unsafe [self as "SBMemoryRegionInfo*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        if ptr.is_null() {
            None
        } else {
            Some(unsafe { get_str(ptr) })
        }
    }
}

impl fmt::Debug for SBMemoryRegionInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBMemoryRegionInfo*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}
