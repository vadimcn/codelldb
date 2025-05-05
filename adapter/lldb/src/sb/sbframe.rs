use super::*;

cpp_class!(pub unsafe struct SBFrame as "SBFrame");

impl SBFrame {
    pub fn function_name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBFrame*"] -> *const c_char as "const char*" {
            return self->GetFunctionName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn display_function_name(&self) -> Option<&str> {
        let ptr = cpp!(unsafe [self as "SBFrame*"] -> *const c_char as "const char*" {
            return self->GetDisplayFunctionName();
        });
        if ptr.is_null() {
            None
        } else {
            unsafe { Some(get_str(ptr)) }
        }
    }
    pub fn symbol(&self) -> SBSymbol {
        cpp!(unsafe [self as "SBFrame*"] -> SBSymbol as "SBSymbol" {
            return self->GetSymbol();
        })
    }
    pub fn function(&self) -> SBFunction {
        cpp!(unsafe [self as "SBFrame*"] -> SBFunction as "SBFunction" {
            return self->GetFunction();
        })
    }
    pub fn line_entry(&self) -> Option<SBLineEntry> {
        cpp!(unsafe [self as "SBFrame*"] -> SBLineEntry as "SBLineEntry" {
            return self->GetLineEntry();
        })
        .check()
    }
    pub fn compile_uint(&self) -> Option<SBCompileUnit> {
        cpp!(unsafe [self as "SBFrame*"] -> SBCompileUnit as "SBCompileUnit" {
            return self->GetCompileUnit();
        })
        .check()
    }
    pub fn module(&self) -> SBModule {
        cpp!(unsafe [self as "SBFrame*"] -> SBModule as "SBModule" {
            return self->GetModule();
        })
    }
    pub fn pc_address(&self) -> SBAddress {
        cpp!(unsafe [self as "SBFrame*"] -> SBAddress as "SBAddress" {
            return self->GetPCAddress();
        })
    }
    pub fn thread(&self) -> SBThread {
        cpp!(unsafe [self as "SBFrame*"] -> SBThread as "SBThread" {
            return self->GetThread();
        })
    }
    pub fn variables(&self, options: &VariableOptions) -> SBValueList {
        let VariableOptions {
            arguments,
            locals,
            statics,
            in_scope_only,
        } = *options;
        cpp!(unsafe [self as "SBFrame*", arguments as "bool", locals as "bool", statics as "bool",
                     in_scope_only as "bool"] -> SBValueList as "SBValueList" {
            return self->GetVariables(arguments, locals, statics, in_scope_only);
        })
    }
    pub fn find_variable(&self, name: &str) -> Option<SBValue> {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBFrame*", name as "const char*"] -> SBValue as "SBValue" {
                return self->FindVariable(name);
            })
        })
        .check()
    }
    pub fn find_value(&self, name: &str, value_type: ValueType) -> Option<SBValue> {
        with_cstr(name, |name| {
            cpp!(unsafe [self as "SBFrame*", name as "const char*", value_type as "ValueType"] -> SBValue as "SBValue" {
                return self->FindValue(name, value_type);
            })
        })
        .check()
    }
    pub fn evaluate_expression(&self, expr: &str) -> SBValue {
        with_cstr(expr, |expr| {
            cpp!(unsafe [self as "SBFrame*", expr as "const char*"] -> SBValue as "SBValue" {
                return self->EvaluateExpression(expr);
            })
        })
    }
    pub fn registers(&self) -> SBValueList {
        cpp!(unsafe [self as "SBFrame*"] -> SBValueList as "SBValueList" {
            return self->GetRegisters();
        })
    }
    pub fn pc(&self) -> Address {
        cpp!(unsafe [self as "SBFrame*"] -> Address as "addr_t" {
            return self->GetPC();
        })
    }
    pub fn set_pc(&self, address: Address) -> bool {
        cpp!(unsafe [self as "SBFrame*", address as "addr_t"] -> bool as "bool" {
            return self->SetPC(address);
        })
    }
    pub fn sp(&self) -> Address {
        cpp!(unsafe [self as "SBFrame*"] -> Address as "addr_t" {
            return self->GetSP();
        })
    }
    pub fn fp(&self) -> Address {
        cpp!(unsafe [self as "SBFrame*"] -> Address as "addr_t" {
            return self->GetFP();
        })
    }
}

impl IsValid for SBFrame {
    fn is_valid(&self) -> bool {
        cpp!(unsafe [self as "SBFrame*"] -> bool as "bool" {
            return self->IsValid();
        })
    }
}

impl fmt::Debug for SBFrame {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        debug_descr(f, |descr| {
            cpp!(unsafe [self as "SBFrame*", descr as "SBStream*"] -> bool as "bool" {
                return self->GetDescription(*descr);
            })
        })
    }
}

#[derive(Clone, Copy, Debug)]
pub struct VariableOptions {
    pub arguments: bool,
    pub locals: bool,
    pub statics: bool,
    pub in_scope_only: bool,
}
