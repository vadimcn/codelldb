use super::*;

cpp_class!(pub unsafe struct SBSymbolContext as "SBSymbolContext");

impl SBSymbolContext {
    pub fn module(&self) -> SBModule {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBModule as "SBModule" {
            return self->GetModule();
        })
    }
    pub fn line_entry(&self) -> SBLineEntry {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntry();
        })
    }
    pub fn symbol(&self) -> SBSymbol {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbol();
        })
    }
    pub fn function(&self) -> SBFunction {
        cpp!(unsafe [self as "SBSymbolContext*"] -> SBFunction as "SBFunction" {
            return self->GetFunction();
        })
    }
    pub fn get_description(&self, description: &mut SBStream) -> bool {
        cpp!(unsafe [self as "SBSymbolContext*", description as "SBStream*"] -> bool as "bool" {
            return self->GetDescription(*description);
        })
    }
}

impl IsValid for SBSymbolContext {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBSymbolContext*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBSymbolContext {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBSymbolContext*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

// These mask bits allow a common interface for queries that can
// limit the amount of information that gets parsed to only the
// information that is requested. These bits also can indicate what
// actually did get resolved during query function calls.
//
// Each definition corresponds to a one of the member variables
// in this class, and requests that that item be resolved, or
// indicates that the member did get resolved.
bitflags! {
    pub struct SymbolContextScope : u32 {
        ///< Set when \a target is requested from
        ///a query, or was located in query
        ///results
        const Target = (1 << 0);
        ///< Set when \a module is requested from
        ///a query, or was located in query
        ///results
        const Module = (1 << 1);
        ///< Set when \a comp_unit is requested
        ///from a query, or was located in query
        ///results
        const CompUnit = (1 << 2);
        ///< Set when \a function is requested
        ///from a query, or was located in query
        ///results
        const Function = (1 << 3);
        ///< Set when the deepest \a block is
        ///requested from a query, or was located
        ///in query results
        const Block = (1 << 4);
        ///< Set when \a line_entry is
        ///requested from a query, or was
        ///located in query results
        const LineEntry = (1 << 5);
        ///< Set when \a symbol is requested from
        ///a query, or was located in query
        ///results
        const Symbol = (1 << 6);
        ///< Indicates to try and lookup everything
        ///up during a routine symbol context
        ///query.
        const Everything = ((1 << 7) - 1);
        ///< Set when \a global or static
        ///variable is requested from a query, or
        ///was located in query results.
        ///< Variable is potentially expensive to lookup so it isn't
        ///included in
        ///< Everything which stops it from being used during frame PC
        ///lookups and
        ///< many other potential address to symbol context lookups.
        const Variable = (1 << 7);
    }
}
