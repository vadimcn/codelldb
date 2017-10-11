import logging
import keyword
import re
import operator
import lldb

log = logging.getLogger('expressions')

__all__ = ['init_formatters', 'analyze', 'PyEvalContext', 'Value', 'find_var_in_frame',
           'preprocess_simple_expr', 'preprocess_python_expr', 'escape_variable_name']

debugger_obj = None
analyzed = {} # A list of type names that we've already analyzed
rust_analyze = None

def init_formatters(debugger):
    global debugger_obj
    debugger_obj = debugger

# Analyze value's type and make sure the appropriate visualizers are attached.
def analyze(sbvalue):
    global analyzed
    global rust_analyze
    qual_type_name = sbvalue.GetType().GetName()
    if qual_type_name in analyzed:
        return
    analyzed[qual_type_name] = True

    #log.info('expressions.analyze for %s %d', sbvalue.GetType().GetName(), sbvalue.GetType().GetTypeClass())
    if not rust_analyze:
        if sbvalue.GetFrame().GetCompileUnit().GetLanguage() != lldb.eLanguageTypeRust:
            return
        from .formatters import rust
        rust.initialize(debugger_obj, analyze)
        rust_analyze = rust.analyze
    rust_analyze(sbvalue)


def find_var_in_frame(sbframe, name):
    val = sbframe.FindVariable(name)
    if not val.IsValid():
        for val_type in [lldb.eValueTypeVariableGlobal,
                         lldb.eValueTypeVariableStatic,
                         lldb.eValueTypeRegister,
                         lldb.eValueTypeConstResult]:
            val = sbframe.FindValue(name, val_type)
            if val.IsValid():
                break
    if not val.IsValid():
        val = sbframe.GetValueForVariablePath(name)
    return val

# A dictionary-like object that fetches values from SBFrame (and caches them).
class PyEvalContext(dict):
    def __init__(self, sbframe):
        self.sbframe = sbframe

    def __missing__(self, name):
        val = find_var_in_frame(self.sbframe, name)
        if val.IsValid():
            val = Value(val)
            self.__setitem__(name, val)
            return val
        else:
            raise KeyError(name)

# A wrapper around SBValue that overloads Python operators to do the right thing (well, mostly).
class Value(object):
    __slots__ = ['__sbvalue']

    def __init__(self, sbvalue):
        self.__sbvalue = sbvalue
        analyze(sbvalue)

    @classmethod
    def unwrap(cls, value):
        return value.__sbvalue if type(value) is Value else value

    def __nonzero__(self):
        return self.__sbvalue.__nonzero__()

    def __str__(self):
        return str(get_value(self))

    def __repr__(self):
        return 'Value(' + str(get_value(self)) + ')'

    def __getitem__(self, key):
        # Allow array access if this value has children...
        if type(key) is Value:
            key = int(key)
        if type(key) is int:
            child_sbvalue = (self.__sbvalue.GetValueForExpressionPath("[%i]" % key))
            if child_sbvalue and child_sbvalue.IsValid():
                return Value(child_sbvalue)
            raise IndexError("Index '%d' is out of range" % key)
        raise TypeError("No array item of type %s" % str(type(key)))

    def __iter__(self):
        return ValueIter(self.__sbvalue)

    def __getattr__(self, name):
        child_sbvalue = self.__sbvalue.GetChildMemberWithName (name)
        if child_sbvalue and child_sbvalue.IsValid():
            return Value(child_sbvalue)
        raise AttributeError("Attribute '%s' is not defined" % name)

    def __neg__(self):
        return -get_value(self)

    def __pos__(self):
        return +get_value(self)

    def __abs__(self):
        return abs(get_value(self))

    def __invert__(self):
        return ~get_value(self)

    def __complex__(self):
        return complex(get_value(self))

    def __int__(self):
        is_num, is_signed, is_float = is_numeric_type(self.__sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num and not is_signed: return self.__sbvalue.GetValueAsUnsigned()
        return self.__sbvalue.GetValueAsSigned()

    def __long__(self):
        return self.__int__()

    def __float__(self):
        is_num, is_signed, is_float = is_numeric_type(self.__sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num and is_float:
            return float(self.__sbvalue.GetValue())
        else:
            return float(self.__sbvalue.GetValueAsSigned())

    def __index__(self):
        return self.__int__()

    def __oct__(self):
        return '0%o' % self.__sbvalue.GetValueAsUnsigned()

    def __hex__(self):
        return '0x%x' % self.__sbvalue.GetValueAsUnsigned()

    def __len__(self):
        return self.__sbvalue.GetNumChildren()

    # On-the-left ops
    def __add__(self, other):
        return get_value(self) + get_value(other)

    def __sub__(self, other):
        return get_value(self) - get_value(other)

    def __mul__(self, other):
        return get_value(self) * get_value(other)

    def __div__(self, other):
        return get_value(self) / get_value(other)

    def __floordiv__(self, other):
        return get_value(self) // get_value(other)

    def __truediv__(self, other):
        return get_value(self) / get_value(other)

    def __mod__(self, other):
        return get_value(self) % get_value(other)

    def __divmod__(self, other):
        return divmod(get_value(self), get_value(other))

    def __pow__(self, other):
        return get_value(self) ** get_value(other)

    def __lshift__(self, other):
        return get_value(self) << get_value(other)

    def __rshift__(self, other):
        return get_value(self) >> get_value(other)

    def __and__(self, other):
        return get_value(self) & get_value(other)

    def __xor__(self, other):
        return get_value(self) ^ get_value(other)

    def __or__(self, other):
        return get_value(self) | get_value(other)

    # On-the-right ops
    def __radd__(self, other):
        return get_value(other) + get_value(self)

    def __rsub__(self, other):
        return get_value(other) - get_value(self)

    def __rmul__(self, other):
        return get_value(other) * get_value(self)

    def __rdiv__(self, other):
        return get_value(other) / get_value(self)

    def __rfloordiv__(self, other):
        return get_value(other) // get_value(self)

    def __rtruediv__(self, other):
        return get_value(other) / get_value(self)

    def __rmod__(self, other):
        return get_value(other) % get_value(self)

    def __rdivmod__(self, other):
        return divmod(get_value(other), get_value(self))

    def __rpow__(self, other):
        return get_value(other) ** get_value(self)

    def __rlshift__(self, other):
        return get_value(other) << get_value(self)

    def __rrshift__(self, other):
        return get_value(other) >> get_value(self)

    def __rand__(self, other):
        return get_value(other) & get_value(self)

    def __rxor__(self, other):
        return get_value(other) ^ get_value(self)

    def __ror__(self, other):
        return get_value(other) | get_value(self)

    # In-place ops
    def __inplace(self, result):
        self.__sbvalue.SetValueFromCString(str(result))
        return result

    def __iadd__(self, other):
        return self.__inplace(self.__add__(other))

    def __isub__(self, other):
        return self.__inplace(self.__sub__(other))

    def __imul__(self, other):
        return self.__inplace(self.__mul__(other))

    def __idiv__(self, other):
        return self.__inplace(self.__div__(other))

    def __itruediv__(self, other):
        return self.__inplace(self.__truediv__(other))

    def __ifloordiv__(self, other):
        return self.__inplace(self.__floordiv__(other))

    def __imod__(self, other):
        return self.__inplace(self.__mod__(other))

    def __ipow__(self, other):
        return self.__inplace(self.__pow__(other))

    def __ilshift__(self, other):
        return self.__inplace(self.__lshift__(other))

    def __irshift__(self, other):
        return self.__inplace(self.__rshift__(other))

    def __iand__(self, other):
        return self.__inplace(self.__and__(other))

    def __ixor__(self, other):
        return self.__inplace(self.__xor__(other))

    def __ior__(self, other):
        return self.__inplace(self.__or__(other))

    # Comparisons
    def __compare(self, other, op):
        if type(other) is int:
            return op(int(self), other)
        elif type(other) is float:
            return op(float(self), other)
        elif type(other) is str:
            return op(str(self), other)
        elif type(other) is Value:
            return op(get_value(self), get_value(other))
        raise TypeError("Unknown type %s, No comparison operation defined." % str(type(other)))

    def __lt__(self, other):
        return self.__compare(other, operator.lt)

    def __le__(self, other):
        return self.__compare(other, operator.le)

    def __gt__(self, other):
        return self.__compare(other, operator.gt)

    def __ge__(self, other):
        return self.__compare(other, operator.ge)

    def __eq__(self, other):
        return self.__compare(other, operator.eq)

    def __ne__(self, other):
        return self.__compare(other, operator.ne)

class ValueIter(object):
    __slots__ = ['index', 'sbvalue', 'length']

    def __init__(self, value):
        self.index = 0
        self.sbvalue = Value.unwrap(value)
        self.length = self.sbvalue.GetNumChildren()

    def __iter__(self):
        return self

    def __next__(self):
        if self.index >= self.length:
            raise StopIteration()
        child_sbvalue = self.sbvalue.GetChildAtIndex(self.index)
        self.index += 1
        return Value(child_sbvalue)

    next = __next__ # PY2 compatibility.

# Converts a Value to an int, a float or a string
def get_value(v):
    if type(v) is Value:
        sbvalue = Value.unwrap(v)
        is_num, is_signed, is_float = is_numeric_type(sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num:
            if is_float:
                return float(sbvalue.GetValue())
            elif is_signed:
                return sbvalue.GetValueAsSigned()
            else:
                return sbvalue.GetValueAsUnsigned()
        else:
            str_val = sbvalue.GetSummary() or ''
            if str_val.startswith('"') and str_val.endswith('"') and len(str_val) > 1:
                str_val = str_val[1:-1]
            return str_val
    else:
        return v # passthrough

# given an lldb.SBBasicType it returns a tuple (is_numeric, is_signed, is_float)
def is_numeric_type(basic_type):
    return type_traits.get(basic_type, (False, False, False))
type_traits = {
    lldb.eBasicTypeInvalid: (False, False, False),
    lldb.eBasicTypeVoid: (False, False, False),
    lldb.eBasicTypeChar: (True, False, False),
    lldb.eBasicTypeSignedChar: (True, True, False),
    lldb.eBasicTypeUnsignedChar: (True, False, False),
    lldb.eBasicTypeWChar: (True, False, False),
    lldb.eBasicTypeSignedWChar: (True, True, False),
    lldb.eBasicTypeUnsignedWChar: (True, False, False),
    lldb.eBasicTypeChar16: (True, False, False),
    lldb.eBasicTypeChar32: (True, False, False),
    lldb.eBasicTypeShort: (True, True, False),
    lldb.eBasicTypeUnsignedShort: (True, False, False),
    lldb.eBasicTypeInt: (True, True, False),
    lldb.eBasicTypeUnsignedInt: (True, False, False),
    lldb.eBasicTypeLong: (True, True, False),
    lldb.eBasicTypeUnsignedLong: (True, False, False),
    lldb.eBasicTypeLongLong: (True, True, False),
    lldb.eBasicTypeUnsignedLongLong: (True, False, False),
    lldb.eBasicTypeInt128: (True, True, False),
    lldb.eBasicTypeUnsignedInt128: (True, False, False),
    lldb.eBasicTypeBool: (False, False, False),
    lldb.eBasicTypeHalf: (True, True, True),
    lldb.eBasicTypeFloat: (True, True, True),
    lldb.eBasicTypeDouble: (True, True, True),
    lldb.eBasicTypeLongDouble: (True, True, True),
    lldb.eBasicTypeFloatComplex: (True, True, False),
    lldb.eBasicTypeDoubleComplex: (True, True, False),
    lldb.eBasicTypeLongDoubleComplex: (True, True, False),
    lldb.eBasicTypeObjCID: (False, False, False),
    lldb.eBasicTypeObjCClass: (False, False, False),
    lldb.eBasicTypeObjCSel: (False, False, False),
    lldb.eBasicTypeNullPtr: (False, False, False),
}

# Matches Python strings
pystring = '|'.join([
    r'(?:"(?:\\"|\\\\|[^"])*")',
    r"(?:'(?:\\'|\\\\|[^'])*')",
    r'(?:r"[^"]*")',
    r"(?:r'[^']*')",
])

# Matches Python keywords
keywords = '|'.join(keyword.kwlist)

# Matches identifiers
ident = r'[A-Za-z_] [A-Za-z0-9_]*'

# Matches `::xxx`, `xxx::yyy`, `::xxx::yyy`, `xxx::yyy::zzz`, etc
qualified_ident = r'(?: (?: ::)? (?: {ident} ::)+ | :: ) {ident}'.format(**locals())

# Matches `xxx`, `::xxx`, `xxx::yyy`, `::xxx::yyy`, `xxx::yyy::zzz`, etc
maybe_qualified_ident = r'(?: ::)? (?: {ident} ::)* {ident}'.format(**locals())

# Matches `$xxx`, `$xxx::yyy::zzz` or `${...}`
escaped_ident = r'\$ ({maybe_qualified_ident}) | \$ {{ ([^}}]*) }}'.format(**locals())

preprocess_simple = r'(\.)? (\b (?:{keywords}) \b | {qualified_ident}) | {escaped_ident} | {pystring}'

preprocess_python = r'(\.)? {escaped_ident} | {pystring}'

maybe_qualified_ident_regex = re.compile('^ {maybe_qualified_ident} $'.format(**locals()), re.X)
preprocess_simple_regex = re.compile(preprocess_simple.format(**locals()), re.X)
preprocess_python_regex = re.compile(preprocess_python.format(**locals()), re.X)

def replacer(match):
    groups = match.groups(None)
    prefix = groups[0]
    for ident in groups[1:]:
        if ident is not None:
            if prefix is None:
                return '__frame_vars["%s"]' % ident
            elif prefix == '.':
                return '.__getattr__("%s")' % ident
            else:
                assert False
    else: # a string - return unchanged
        return match.group(0)

# Replaces identifiers that are invalid according to Python syntax in simple expressions:
# - identifiers that happen to be Python keywords (e.g.`for`),
# - qualified identifiers (e.g. `foo::bar::baz`),
# - raw identifiers if the form $xxxxxx,
# with access via `__frame_vars`, or `__getattr__()` (if prefixed by a dot).
# For example, `for + foo::bar::baz + foo::bar::baz.class() + $SomeClass<int>::value` will be translated into
# `__frame_vars["for"] + __frame_vars["foo::bar::baz"] +
#  __frame_vars["foo::bar::baz"].__getattr__("class") + __frame_vars["SomeClass<int>::value"]`
def preprocess_simple_expr(expr):
    return preprocess_simple_regex.sub(replacer, expr)

# Replaces variable placeholders in native Python expressions with access via __frame_vars,
# or `__getattr__()` (if prefixed by a dot).
# For example, `$var + 42` will be translated into `__frame_vars["var"] + 42`.
def preprocess_python_expr(expr):
    return preprocess_python_regex.sub(replacer, expr)

def escape_variable_name(name):
    if maybe_qualified_ident_regex.match(name) is not None:
        return name
    else:
        return '${' + name + '}'

# --- Tests ---

def compare(expected, actual):
    if expected != actual:
        print('EXPECTED:'); print(expected)
        print('ACTUAL:'); print(actual)
        raise AssertionError('expected != actual')

def test_preprocess_simple():
    expr = """
        class = from.global.finally
        local::bar::__BAZ()
        local_string()
        ::foo
        ::foo::bar::baz
        foo::bar::baz
        $local::foo::bar
        ${std::integral_constant<long, 1l>::value}
        ${std::integral_constant<long, 1l, foo<123>>::value}
        ${std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value}
        '''continue.exec = pass.print; yield.with = 3'''
        "continue.exec = pass.print; yield.with = 3"
    """

    expected = """
        __frame_vars["class"] = __frame_vars["from"].__getattr__("global").__getattr__("finally")
        __frame_vars["local::bar::__BAZ"]()
        local_string()
        __frame_vars["::foo"]
        __frame_vars["::foo::bar::baz"]
        __frame_vars["foo::bar::baz"]
        __frame_vars["local::foo::bar"]
        __frame_vars["std::integral_constant<long, 1l>::value"]
        __frame_vars["std::integral_constant<long, 1l, foo<123>>::value"]
        __frame_vars["std::allocator_traits<std::allocator<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> > > >::__construct_helper<std::thread::_Impl<std::_Bind_simple<threads(int)::__lambda0(int)> >, std::_Bind_simple<threads(int)::__lambda0(int)> >::value"]
        '''continue.exec = pass.print; yield.with = 3'''
        "continue.exec = pass.print; yield.with = 3"
    """
    prepr = preprocess_simple_expr(expr)
    compare(expected, prepr)

def test_preprocess_python():
    expr = """
        for x in $foo: print x
        $xxx.$yyy.$zzz
        $xxx::yyy::zzz
        $::xxx
        "$xxx::yyy::zzz"
    """
    expected = """
        for x in __frame_vars["foo"]: print x
        __frame_vars["xxx"].__getattr__("yyy").__getattr__("zzz")
        __frame_vars["xxx::yyy::zzz"]
        __frame_vars["::xxx"]
        "$xxx::yyy::zzz"
    """
    prepr = preprocess_python_expr(expr)
    compare(expected, prepr)

def test_escape_variable_name():
    assert escape_variable_name('foo') == 'foo'
    assert escape_variable_name('foo::bar') == 'foo::bar'
    assert escape_variable_name('foo::bar<34>') == '${foo::bar<34>}'
    assert escape_variable_name('foo::bar<34>::value') == '${foo::bar<34>::value}'

def run_tests():
    test_preprocess_simple()
    test_preprocess_python()
    test_escape_variable_name()
