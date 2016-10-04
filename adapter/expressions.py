import lldb

class Value(object):
    def __init__(self, sbvalue):
        self.sbvalue = sbvalue

    def __nonzero__(self):
        return self.sbvalue.__nonzero__()

    def __str__(self):
        return str(self.sbvalue.GetValue())

    def __getitem__(self, key):
        # Allow array access if this value has children...
        if type(key) is Value:
            key = int(key)
        if type(key) is int:
            child_sbvalue = (self.sbvalue.GetValueForExpressionPath("[%i]" % key))
            if child_sbvalue and child_sbvalue.IsValid():
                return Value(child_sbvalue)
            raise IndexError("Index '%d' is out of range" % key)
        raise TypeError("No array item of type %s" % str(type(key)))

    def __iter__(self):
        return ValueIter(self.sbvalue)

    def __getattr__(self, name):
        child_sbvalue = self.sbvalue.GetChildMemberWithName (name)
        if child_sbvalue and child_sbvalue.IsValid():
            return Value(child_sbvalue)
        raise AttributeError("Attribute '%s' is not defined" % name)

    def __add__(self, other):
        return get_value(self) + get_value(other)

    def __sub__(self, other):
        return get_value(self) - get_value(other)

    def __mul__(self, other):
        return get_value(self) * get_value(other)

    def __floordiv__(self, other):
        return get_value(self) // get_value(other)

    def __mod__(self, other):
        return get_value(self) %get_value(other)

    def __divmod__(self, other):
        return get_value(self) % get_value(other)

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

    def __div__(self, other):
        return get_value(self) / get_value(other)

    def __truediv__(self, other):
        return get_value(self) / get_value(other)

    def __iadd__(self, other):
        result = self.__add__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __isub__(self, other):
        result = self.__sub__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __imul__(self, other):
        result = self.__mul__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __idiv__(self, other):
        result = self.__div__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __itruediv__(self, other):
        result = self.__truediv__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ifloordiv__(self, other):
        result =  self.__floordiv__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __imod__(self, other):
        result =  self.__and__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ipow__(self, other):
        result = self.__pow__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ipow__(self, other, modulo):
        result = self.__pow__(self, other, modulo)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ilshift__(self, other):
        result = self.__lshift__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __irshift__(self, other):
        result =  self.__rshift__(other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __iand__(self, other):
        result =  self.__and__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ixor__(self, other):
        result =  self.__xor__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

    def __ior__(self, other):
        result =  self.__ior__(self, other)
        self.sbvalue.SetValueFromCString(str(result))
        return result

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
        is_num, is_signed, is_float = is_numeric_type(self.sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num and not is_signed: return self.sbvalue.GetValueAsUnsigned()
        return self.sbvalue.GetValueAsSigned()

    def __long__(self):
        return self.__int__()

    def __float__(self):
        is_num, is_signed, is_float = is_numeric_type(self.sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num and is_float:
            return float(self.sbvalue.GetValue())
        else:
            return float(self.sbvalue.GetValueAsSigned())
    
    def __index__(self):
        return self.__int__()

    def __oct__(self):
        return '0%o' % self.sbvalue.GetValueAsUnsigned()

    def __hex__(self):
        return '0x%x' % self.sbvalue.GetValueAsUnsigned()

    def __len__(self):
        return self.sbvalue.GetNumChildren()

    def __eq__(self, other):
        if type(other) is int:
            return int(self) == other
        elif type(other) is str:
            return str(self) == other
        elif type(other) is Value:
            return get_value(self) == get_value(other)
        raise TypeError("Unknown type %s, No equality operation defined." % str(type(other)))

    def __ne__(self, other):
        return not self.__eq__(other)

class ValueIter(object):
    def __init__(self,Value):
        self.index = 0
        self.sbvalue = Value
        if type(self.sbvalue) is Value:
            self.sbvalue = self.sbvalue.sbvalue
        self.length = self.sbvalue.GetNumChildren()

    def __iter__(self):
        return self

    def next(self):
        if self.index >= self.length:
            raise StopIteration()
        child_sbvalue = self.sbvalue.GetChildAtIndex(self.index)
        self.index += 1
        return Value(child_sbvalue)

def get_value(v):
    if type(v) is Value:
        is_num, is_signed, is_float = is_numeric_type(v.sbvalue.GetType().GetCanonicalType().GetBasicType())
        if is_num:
            if is_float:
                return float(v.sbvalue.GetValue())
            elif is_signed:
                return v.sbvalue.GetValueAsSigned()
            else:
                return v.sbvalue.GetValueAsUnsigned()
        else:
            str_val = v.sbvalue.GetSummary()
            if str_val.startswith('"') and str_val.endswith('"') and len(str_val) > 1:
                str_val = str_val[1:-1]
            return str_val 
    else:
        return v

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
