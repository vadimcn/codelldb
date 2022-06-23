import lldb
import operator


class Value(object):
    '''A wrapper around SBValue that implements standard Python operators.'''
    __slots__ = ['__sbvalue']

    def __init__(self, sbvalue):
        self.__sbvalue = sbvalue

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
        if not isinstance(key, slice):
            child_sbvalue = (self.__sbvalue.GetValueForExpressionPath("[%i]" % operator.index(key)))
            if child_sbvalue and child_sbvalue.IsValid():
                return Value(child_sbvalue)
            raise IndexError("Index '%d' is out of range" % key)
        else:
            return [self[i] for i in range(*key.indices(len(self)))]

    def __iter__(self):
        return ValueIter(self.__sbvalue)

    def __getattr__(self, name):
        child_sbvalue = self.__sbvalue.GetChildMemberWithName(name)
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
        is_num, is_signed, is_float = is_numeric_type(self.__sbvalue)
        if is_num and is_signed:
            return self.__sbvalue.GetValueAsSigned()
        else:
            return self.__sbvalue.GetValueAsUnsigned()

    def __long__(self):
        return self.__int__()

    def __float__(self):
        is_num, is_signed, is_float = is_numeric_type(self.__sbvalue)
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

    def __contains__(self, other):
        return get_value(self).__contains__(other)

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
    def __lt__(self, other):
        return get_value(self) < get_value(other)

    def __le__(self, other):
        return get_value(self) <= get_value(other)

    def __gt__(self, other):
        return get_value(self) > get_value(other)

    def __ge__(self, other):
        return get_value(self) >= get_value(other)

    def __eq__(self, other):
        return get_value(self) == get_value(other)

    def __ne__(self, other):
        return get_value(self) != get_value(other)


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

    next = __next__  # PY2 compatibility.


def get_value(v):
    '''Convert a Value to an int, a float or a string'''
    if type(v) is Value:
        sbvalue = Value.unwrap(v)
        is_num, is_signed, is_float = is_numeric_type(sbvalue)
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
        return v  # passthrough


def is_numeric_type(sbvalue):
    return type_traits.get(sbvalue.GetType().GetCanonicalType().GetBasicType(), (False, False, False))


# lldb.SBBasicType => (is_numeric, is_signed, is_float)
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
