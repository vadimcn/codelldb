import lldb
import debugger


def eval(expr):
    return debugger.evaluate(expr, unwrap=True)


def show_type(ty):
    if isinstance(ty, str):
        ty = eval(ty).GetType()
    print('Name:', ty.GetName())
    print('TypeClass:', str_type_class(ty.GetTypeClass()))
    print('BasicType:', str_basic_type(ty.GetBasicType()))
    print('Number of template arguments:', ty.GetNumberOfTemplateArguments())
    for i in range(ty.GetNumberOfTemplateArguments()):
        print('  {} {}'.format(str_templ_arg_kind(ty.GetTemplateArgumentKind(i)),
                               ty.GetTemplateArgumentType(i).GetName()))


def show_value(val):
    if isinstance(val, str):
        val = eval(val)
    print('Name:', val.GetName())
    print('Value:', val.GetValue())
    print('Summary:', val.GetSummary())
    print('TypeName:', val.GetTypeName())
    print('ValueType:', str_value_type(val.GetValueType()))
    print('IsSynthetic:', val.IsSynthetic())


type_classes = [
    (lldb.eTypeClassArray, 'Array'),
    (lldb.eTypeClassBlockPointer, 'BlockPointer'),
    (lldb.eTypeClassBuiltin, 'Builtin'),
    (lldb.eTypeClassClass, 'Class'),
    (lldb.eTypeClassComplexFloat, 'ComplexFloat'),
    (lldb.eTypeClassComplexInteger, 'ComplexInteger'),
    (lldb.eTypeClassEnumeration, 'Enumeration'),
    (lldb.eTypeClassFunction, 'Function'),
    (lldb.eTypeClassMemberPointer, 'MemberPointer'),
    (lldb.eTypeClassObjCObject, 'ObjCObject'),
    (lldb.eTypeClassObjCInterface, 'ObjCInterface'),
    (lldb.eTypeClassObjCObjectPointer, 'ObjCObjectPointer'),
    (lldb.eTypeClassPointer, 'Pointer'),
    (lldb.eTypeClassReference, 'Reference'),
    (lldb.eTypeClassStruct, 'Struct'),
    (lldb.eTypeClassTypedef, 'Typedef'),
    (lldb.eTypeClassUnion, 'Union'),
    (lldb.eTypeClassVector, 'Vector'),
    (lldb.eTypeClassOther, 'Other'),
]


def str_type_class(tc):
    if tc == lldb.eTypeClassInvalid:
        return 'Invalid'
    elif tc == lldb.eTypeClassAny:
        return 'Any'

    cls = []
    for val, name in type_classes:
        if tc & val:
            cls.append(name)
    return '|'.join(cls)


val_types = [
    (lldb.eValueTypeInvalid, 'Invalid'),
    (lldb.eValueTypeVariableGlobal, 'VariableGlobal'),
    (lldb.eValueTypeVariableStatic, 'VariableStatic'),
    (lldb.eValueTypeVariableArgument, 'VariableArgument'),
    (lldb.eValueTypeVariableLocal, 'VariableLocal'),
    (lldb.eValueTypeRegister, 'Register'),
    (lldb.eValueTypeRegisterSet, 'RegisterSet'),
    (lldb.eValueTypeConstResult, 'ConstResult'),
    (lldb.eValueTypeVariableThreadLocal, 'VariableThreadLocal'),
]


def str_value_type(vt):
    for val, name in val_types:
        if vt == val:
            return name
    return '?'


basic_types = [
    (lldb.eBasicTypeInvalid, 'Invalid'),
    (lldb.eBasicTypeVoid, 'Void'),
    (lldb.eBasicTypeChar, 'Char'),
    (lldb.eBasicTypeSignedChar, 'SignedChar'),
    (lldb.eBasicTypeUnsignedChar, 'UnsignedChar'),
    (lldb.eBasicTypeWChar, 'WChar'),
    (lldb.eBasicTypeSignedWChar, 'SignedWChar'),
    (lldb.eBasicTypeUnsignedWChar, 'UnsignedWChar'),
    (lldb.eBasicTypeChar16, 'Char16'),
    (lldb.eBasicTypeChar32, 'Char32'),
    (lldb.eBasicTypeShort, 'Short'),
    (lldb.eBasicTypeUnsignedShort, 'UnsignedShort'),
    (lldb.eBasicTypeInt, 'Int'),
    (lldb.eBasicTypeUnsignedInt, 'UnsignedInt'),
    (lldb.eBasicTypeLong, 'Long'),
    (lldb.eBasicTypeUnsignedLong, 'UnsignedLong'),
    (lldb.eBasicTypeLongLong, 'LongLong'),
    (lldb.eBasicTypeUnsignedLongLong, 'UnsignedLongLong'),
    (lldb.eBasicTypeInt128, 'Int128'),
    (lldb.eBasicTypeUnsignedInt128, 'UnsignedInt128'),
    (lldb.eBasicTypeBool, 'Bool'),
    (lldb.eBasicTypeHalf, 'Half'),
    (lldb.eBasicTypeFloat, 'Float'),
    (lldb.eBasicTypeDouble, 'Double'),
    (lldb.eBasicTypeLongDouble, 'LongDouble'),
    (lldb.eBasicTypeFloatComplex, 'FloatComplex'),
    (lldb.eBasicTypeDoubleComplex, 'DoubleComplex'),
    (lldb.eBasicTypeLongDoubleComplex, 'LongDoubleComplex'),
    (lldb.eBasicTypeObjCID, 'ObjCID'),
    (lldb.eBasicTypeObjCClass, 'ObjCClass'),
    (lldb.eBasicTypeObjCSel, 'ObjCSel'),
    (lldb.eBasicTypeNullPtr, 'NullPtr'),
    (lldb.eBasicTypeOther, 'Other'),
]


def str_basic_type(vt):
    for val, name in basic_types:
        if vt == val:
            return name
    return '?'


templ_arg_kinds = [
    (lldb.eTemplateArgumentKindNull, 'Null'),
    (lldb.eTemplateArgumentKindType, 'Type'),
    (lldb.eTemplateArgumentKindDeclaration, 'Declaration'),
    (lldb.eTemplateArgumentKindIntegral, 'Integral'),
    (lldb.eTemplateArgumentKindTemplate, 'Template'),
    (lldb.eTemplateArgumentKindTemplateExpansion, 'TemplateExpansion'),
    (lldb.eTemplateArgumentKindExpression, 'Expression'),
    (lldb.eTemplateArgumentKindPack, 'Pack'),
    (lldb.eTemplateArgumentKindNullPtr, 'NullPtr'),
    #(lldb.eTemplateArgumentKindUncommonValue, 'UncommonValue'),
]


def str_templ_arg_kind(ak):
    for val, name in templ_arg_kinds:
        if ak == val:
            return name
    return '?'
