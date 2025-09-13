use super::*;

cpp_class!(pub unsafe struct SBSection as "SBSection");

unsafe impl Send for SBSection {}

impl SBSection {
    pub fn section_type(&self) -> SectionType {
        cpp!(unsafe [self as "SBSection*"] -> u32 as "uint32_t" {
            return self->GetSectionType();
        })
        .into()
    }
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBSection*"] -> *const c_char  as "const char*" {
            return self->GetName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn file_address(&self) -> Address {
        cpp!(unsafe [self as "SBSection*"] -> Address as "lldb::addr_t" {
            return self->GetFileAddress();
        })
    }
    pub fn load_address(&self, target: &SBTarget) -> Address {
        cpp!(unsafe [self as "SBSection*", target as "SBTarget*"] -> Address as "lldb::addr_t" {
            return self->GetLoadAddress(*target);
        })
    }
    pub fn byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBSection*"] -> usize as "size_t" {
            return self->GetByteSize();
        })
    }
    pub fn file_byte_size(&self) -> usize {
        cpp!(unsafe [self as "SBSection*"] -> usize as "size_t" {
            return self->GetFileByteSize();
        })
    }
}

impl IsValid for SBSection {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBSection*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBSection {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBSection*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(u32)]
pub enum SectionType {
    #[default]
    Invalid = 0,
    Code,
    Container,
    ///< The section contains child sections
    Data,
    DataCString,
    ///< Inlined C string data
    DataCStringPointers,
    ///< Pointers to C string data
    DataSymbolAddress,
    ///< Address of a symbol in the symbol table
    Data4,
    Data8,
    Data16,
    DataPointers,
    Debug,
    ZeroFill,
    DataObjCMessageRefs,
    ///< Pointer to function pointer + selector
    DataObjCCFStrings,
    ///< Objective-C const CFString/NSString
    ///< objects
    DWARFDebugAbbrev,
    DWARFDebugAddr,
    DWARFDebugAranges,
    DWARFDebugCuIndex,
    DWARFDebugFrame,
    DWARFDebugInfo,
    DWARFDebugLine,
    DWARFDebugLoc,
    DWARFDebugMacInfo,
    DWARFDebugMacro,
    DWARFDebugPubNames,
    DWARFDebugPubTypes,
    DWARFDebugRanges,
    DWARFDebugStr,
    DWARFDebugStrOffsets,
    DWARFAppleNames,
    DWARFAppleTypes,
    DWARFAppleNamespaces,
    DWARFAppleObjC,
    ELFSymbolTable,
    ///< Elf SHT_SYMTAB section
    ELFDynamicSymbols,
    ///< Elf SHT_DYNSYM section
    ELFRelocationEntries,
    ///< Elf SHT_REL or SHT_REL section
    ELFDynamicLinkInfo,
    ///< Elf SHT_DYNAMIC section
    EHFrame,
    ARMexidx,
    ARMextab,
    CompactUnwind,
    ///< compact unwind section in Mach-O,
    ///< __TEXT,__unwind_info
    TypeGoSymtab,
    TypeAbsoluteAddress,
    ///< Dummy section for symbols with absolute
    ///< address
    DWARFGNUDebugAltLink,
    DWARFDebugTypes,
    ///< DWARF .debug_types section
    DWARFDebugNames,
    ///< DWARF v5 .debug_names
    Other,
    DWARFDebugLineStr,
    ///< DWARF v5 .debug_line_str
    DWARFDebugRngLists,
    ///< DWARF v5 .debug_rnglists
    DWARFDebugLocLists,
    ///< DWARF v5 .debug_loclists
    DWARFDebugAbbrevDwo,
    DWARFDebugInfoDwo,
    DWARFDebugStrDwo,
    DWARFDebugStrOffsetsDwo,
    DWARFDebugTypesDwo,
    DWARFDebugRngListsDwo,
    DWARFDebugLocDwo,
    DWARFDebugLocListsDwo,
    DWARFDebugTuIndex,
}
