use super::*;

cpp_class!(pub unsafe struct SBSymbol as "SBSymbol");

unsafe impl Send for SBSymbol {}

impl SBSymbol {
    pub fn name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBSymbol*"] -> *const c_char as "const char*" {
            return self->GetName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn display_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBSymbol*"] -> *const c_char as "const char*" {
            return self->GetDisplayName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn mangled_name(&self) -> &str {
        let ptr = cpp!(unsafe [self as "SBSymbol*"] -> *const c_char as "const char*" {
            return self->GetMangledName();
        });
        unsafe { get_str(ptr) }
    }
    pub fn symbol_type(&self) -> SymbolType {
        cpp!(unsafe [self as "SBSymbol*"] -> u32 as "uint32_t" {
            return self->GetType();
        })
        .into()
    }
    pub fn start_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBSymbol*"] -> SBAddress as "SBAddress" {
            return self->GetStartAddress();
        })
    }
    pub fn end_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBSymbol*"] -> SBAddress as "SBAddress" {
            return self->GetEndAddress();
        })
    }
    pub fn instructions(&self, target: &SBTarget) -> SBInstructionList {
        let target = target.clone();
        cpp!(unsafe [self as "SBSymbol*", target as "SBTarget"] -> SBInstructionList as "SBInstructionList" {
            return self->GetInstructions(target);
        })
    }
    pub fn get_description(&self, description: &mut SBStream) -> bool {
        cpp!(unsafe [self as "SBSymbol*", description as "SBStream*"] -> bool as "bool" {
            return self->GetDescription(*description);
        })
    }
}

impl IsValid for SBSymbol {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBSymbol*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBSymbol {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBSymbol*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Eq, PartialEq, Debug, FromPrimitive)]
#[repr(u32)]
pub enum SymbolType {
    Any = 0,
    Absolute,
    Code,
    Resolver,
    Data,
    Trampoline,
    Runtime,
    Exception,
    SourceFile,
    HeaderFile,
    ObjectFile,
    CommonBlock,
    Block,
    Local,
    Param,
    Variable,
    VariableType,
    LineEntry,
    LineHeader,
    ScopeBegin,
    ScopeEnd,
    // When symbols take more than one entry, the extra entries get this type
    Additional,
    Compiler,
    Instrumentation,
    Undefined,
    ObjCClass,
    ObjCMetaClass,
    ObjCIVar,
    ReExported,
    #[default]
    Invalid,
}
