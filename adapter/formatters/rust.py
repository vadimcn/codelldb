import logging
import lldb
import codecs
import re

log = logging.getLogger('rust')

def initialize(debugger):
    log.info('register_providers')
    global rust_category
    debugger.HandleCommand('command script import adapter.formatters.rust')
    rust_category = debugger.CreateCategory('Rust')
    rust_category.AddLanguage(lldb.eLanguageTypeRust)
    rust_category.SetEnabled(True)

    attach_summary_to_type('get_str_slice_summary', '&str')
    attach_synthetic_to_type('SliceSynthProvider', '&str')

    attach_summary_to_type('get_string_summary', 'collections::string::String')
    attach_synthetic_to_type('StdStringSynthProvider', 'collections::string::String')

    attach_summary_to_type('get_array_summary', '^.*\\[[0-9]+\\]$', True)

    attach_summary_to_type('get_vector_summary', '^collections::vec::Vec<.+>$', True)
    attach_synthetic_to_type('StdVectorSynthProvider', '^collections::vec::Vec<.+>$', True)

    attach_summary_to_type('get_slice_summary', '^&(mut\\s*)?\\[.*\\]$', True)
    attach_synthetic_to_type('SliceSynthProvider', '^&(mut\\s*)?\\[.*\\]$', True)

    attach_summary_to_type('get_cstring_summary', 'std::ffi::c_str::CString')
    attach_synthetic_to_type('StdCStringSynthProvider', 'std::ffi::c_str::CString')

    attach_summary_to_type('get_osstring_summary', 'std::ffi::os_str::OsString')
    attach_synthetic_to_type('StdOsStringSynthProvider', 'std::ffi::os_str::OsString')

    attach_summary_to_type('rust_summary_provider', '.*', True)

def attach_summary_to_type(summary_fn, obj_type, is_regex=False):
    summary = lldb.SBTypeSummary.CreateWithFunctionName('adapter.formatters.rust.' + summary_fn)
    summary.SetOptions(lldb.eTypeOptionCascade)
    assert rust_category.AddTypeSummary(lldb.SBTypeNameSpecifier(obj_type, is_regex), summary)

def attach_synthetic_to_type(synth_class, obj_type, is_regex=False):
    synth = lldb.SBTypeSynthetic.CreateWithClassName('adapter.formatters.rust.' + synth_class)
    synth.SetOptions(lldb.eTypeOptionCascade)
    assert rust_category.AddTypeSynthetic(lldb.SBTypeNameSpecifier(obj_type, is_regex), synth)

def gcm(valobj, *chain):
    for name in chain:
        valobj = valobj.GetChildMemberWithName(name)
    return valobj

def string_from_ptr(pointer, length):
    error = lldb.SBError()
    process = pointer.GetProcess()
    data = process.ReadMemory(pointer.GetValueAsUnsigned(), length, error)
    if error.Success():
        return data.decode(encoding='UTF-8')
    else:
        log.error('%s', error.GetCString())

# 'get_summary' is annoyingly not a part of the standard LLDB synth provider API.
# This trick allows us to share data extraction logic between synth providers and their
# sibling summary providers.
def get_synth_summary(synth_class, valobj, dict):
    synth = synth_class(valobj.GetNonSyntheticValue(), dict)
    synth.update()
    return synth.get_summary()

def print_array_elements(valobj, maxsize=32):
    s = ''
    for i in range(0, valobj.GetNumChildren()):
        if len(s) > 0: s += ', '
        summary = valobj.GetChildAtIndex(i).GetSummary()
        s += summary if summary is not None else '?'
        if len(s) > maxsize:
            s += ' ...'
            break
    return s

# Enums and tuples cannot be recognized based on type name.  For those we have this catch-all
# summary provider which performs deeper analysis.
ENCODED_ENUM_PREFIX = 'RUST$ENCODED$ENUM$'
ENUM_DISR_FIELD_NAME = 'RUST$ENUM$DISR'
def rust_summary_provider(valobj, dict):
    log.info('rust_summary_provider for %s %d', valobj.GetType().GetName(), valobj.GetType().GetTypeClass())
    try:
        obj_type = valobj.GetType().GetUnqualifiedType()
        type_class = obj_type.GetTypeClass()
        if type_class == lldb.eTypeClassUnion:
            return get_enum_summary(valobj, dict)
        elif type_class == lldb.eTypeClassPointer or type_class == lldb.eTypeClassReference:
            return get_pointer_summary(valobj, dict)
        elif type_class == lldb.eTypeClassStruct:
            if obj_type.GetFieldAtIndex(0).GetName() == ENUM_DISR_FIELD_NAME:
                return get_enum_variant_summary(valobj, dict)
            elif obj_type.GetFieldAtIndex(0).GetName() == '__0':
                return get_tuple_summary(valobj, dict)
        val = valobj.GetValue()
        if val is None:
            val = get_unqualified_type_name(valobj.GetType().GetName())
        return val
    except Exception as e:
        log.error('summary provider error: %s', str(e))
        raise

def get_enum_summary(valobj, dict):
    valobj = valobj.GetNonSyntheticValue()
    obj_type = valobj.GetType()
    num_fields = obj_type.GetNumberOfFields()
    if num_fields == 0:
        return '()'
    elif num_fields == 1:
        first_variant_name = obj_type.GetFieldAtIndex(0).GetName()
        if first_variant_name is None:
            # Singleton
            return valobj.GetChildAtIndex(0)
        else:
            assert first_variant_name.startswith(ENCODED_ENUM_PREFIX)
            # 'Compressed' enums always have two variants, of which one contains no data,
            # and the other one contains a field (not necessarily at the top level) that implements
            # Zeroable.  This field is then used as a two-state discriminant.
            attach_synthetic_to_type('CompressedEnumProvider', obj_type.GetName())
            return get_synth_summary(CompressedEnumProvider, valobj, dict)
    else:
        # Regular enums are represented as unions of structs, containing the discriminant in the
        # first field.
        discriminant = valobj.GetChildAtIndex(0).GetChildAtIndex(0).GetValueAsUnsigned()
        variant = valobj.GetChildAtIndex(discriminant)
        attach_synthetic_to_type('RegularEnumProvider', obj_type.GetName())
        return variant.GetSummary()

def get_enum_variant_summary(valobj, dict):
    obj_type = valobj.GetType()
    num_fields = obj_type.GetNumberOfFields()
    unqual_type_name = get_unqualified_type_name(obj_type.GetName())
    if num_fields == 1:
        return unqual_type_name
    elif obj_type.GetFieldAtIndex(1).GetName().startswith('__'):
        fields = ', '.join([valobj.GetChildAtIndex(i).GetSummary() for i in range(1, num_fields)])
        return '%s(%s)' % (unqual_type_name, fields)
    else:
        fields = [valobj.GetChildAtIndex(i) for i in range(1, num_fields)]
        fields = ', '.join(['%s:%s' % (f.GetName(), f.GetSummary()) for f in fields])
        return '%s{%s}' % (unqual_type_name, fields)

def get_tuple_summary(valobj, dict):
    fields = [valobj.GetChildAtIndex(i).GetSummary() for i in range(0, valobj.GetNumChildren())]
    return '(%s)' % ', '.join(fields)

def get_str_slice_summary(valobj, dict):
    return get_synth_summary(StrSliceSynthProvider, valobj, dict)

def get_string_summary(valobj, dict):
    return get_synth_summary(StdStringSynthProvider, valobj, dict)

def get_cstring_summary(valobj, dict):
    return get_synth_summary(StdCStringSynthProvider, valobj, dict)

def get_osstring_summary(valobj, dict):
    return get_synth_summary(StdOsStringSynthProvider, valobj, dict)

def get_vector_summary(valobj, dict):
    return 'vec![%s]' % print_array_elements(valobj)

def get_array_summary(valobj, dict):
    log.info('get_array_summary %d', valobj.GetNumChildren())
    return '[%s]' % print_array_elements(valobj)

def get_slice_summary(valobj, dict):
    log.info('get_slice_summary %d', valobj.GetNumChildren())
    return '&[%s]' % print_array_elements(valobj)

def get_pointer_summary(valobj, dict):
    return valobj.Dereference().Summary()

UNQUAL_TYPE_MARKERS = ["(", "[", "&", "*"]
UNQUAL_TYPE_REGEX = re.compile('^(?:[A-Za-z0-9]+::)*([A-Za-z0-9]+).*')
def get_unqualified_type_name(type_name):
    if type_name[0] in UNQUAL_TYPE_MARKERS:
        return type_name
    return UNQUAL_TYPE_REGEX.match(type_name).group(1)

class CompressedEnumProvider:
    def __init__(self, valobj, dict):
        self.valobj = valobj
        variant_name = valobj.GetType().GetFieldAtIndex(0).GetName()
        last_separator_index = variant_name.rfind("$")
        start_index = len(ENCODED_ENUM_PREFIX)
        indices_substring = variant_name[start_index:last_separator_index].split("$")
        self.disr_field_indices = [int(index) for index in indices_substring]
        self.null_variant_name = variant_name[last_separator_index + 1:]

    def update(self):
        discriminant = self.valobj.GetChildAtIndex(0)
        for disr_field_index in self.disr_field_indices:
            discriminant = discriminant.GetChildAtIndex(disr_field_index)
        # If the discriminant field is a fat pointer we consider only the first word
        if discriminant.GetType().GetTypeClass() == lldb.eTypeClassStruct:
            discriminant = discriminant.GetChildAtIndex(0)
        self.is_null_variant = discriminant.GetValueAsUnsigned() == 0
        if not self.is_null_variant:
            self.variant = self.valobj.GetChildAtIndex(0)

    def num_children(self):
        return 0 if self.is_null_variant else self.variant.GetNumChildren()

    def has_children(self):
        return False if self.is_null_variant else  self.variant.MightHaveChildren()

    def get_child_at_index(self, index):
        return self.variant.GetChildAtIndex(index)

    def get_child_index(self, name):
        return self.variant.GetIndexOfChildWithName(name)

    def get_summary(self):
        if self.is_null_variant:
            return self.null_variant_name
        else:
            unqual_type_name = get_unqualified_type_name(self.valobj.GetChildAtIndex(0).GetType().GetName())
            return '%s%s' % (unqual_type_name, self.variant.GetSummary())

class RegularEnumProvider:
    def __init__(self, valobj, dict):
        self.valobj = valobj

    def update(self):
        discriminant = self.valobj.GetChildAtIndex(0).GetChildAtIndex(0).GetValueAsUnsigned()
        self.variant = self.valobj.GetChildAtIndex(discriminant)

    def num_children(self):
        return self.variant.GetNumChildren() - 1

    def has_children(self):
        return self.num_children() > 0

    def get_child_at_index(self, index):
        return self.variant.GetChildAtIndex(index + 1)

    def get_child_index(self, name):
        return self.variant.GetIndexOfChildWithName(name) - 1

# Base class for providers that represent array-like objects
class ArrayLikeSynthProvider:
    def __init__(self, valobj, dict):
        self.valobj = valobj

    def update(self):
        try:
            self._update() # Should be overridden
            self.item_type = self.ptr.GetType().GetPointeeType()
            self.item_size = self.item_type.GetByteSize()
        except Exception as e:
            log.error('%s', e)
            raise

    def num_children(self):
        return self.len

    def has_children(self):
        return True

    def get_child_at_index(self, index):
        try:
            if not 0 <= index < self.len:
                return None
            offset = index * self.item_size
            return self.ptr.CreateChildAtOffset('['+str(index)+']', offset, self.item_type)
        except Exception as e:
            log.error('%s', e)
            raise

    def get_child_index(self, name):
        try:
            return int(name.lstrip('[').rstrip(']'))
        except Exception as e:
            log.error('%s', e)
            raise

class StdVectorSynthProvider(ArrayLikeSynthProvider):
    def _update(self):
        self.len = gcm(self.valobj, 'len').GetValueAsUnsigned()
        self.ptr = gcm(self.valobj, 'buf', 'ptr', 'pointer', '__0')

class SliceSynthProvider(ArrayLikeSynthProvider):
    def _update(self):
        valobj = self.valobj
        self.len = gcm(valobj, 'length').GetValueAsUnsigned()
        self.ptr = gcm(valobj, 'data_ptr')

# Base class for *String providers
class StringLikeSynthProvider(ArrayLikeSynthProvider):
    def get_summary(self):
        return '"%s"' % string_from_ptr(self.ptr, self.len)

class StrSliceSynthProvider(StringLikeSynthProvider):
     def _update(self):
        valobj = self.valobj
        self.len = gcm(valobj, 'length').GetValueAsUnsigned()
        self.ptr = gcm(valobj, 'data_ptr')

class StdStringSynthProvider(StringLikeSynthProvider):
    def _update(self):
        vec = gcm(self.valobj, 'vec')
        self.len = gcm(vec, 'len').GetValueAsUnsigned()
        self.ptr = gcm(vec, 'buf', 'ptr', 'pointer', '__0')

class StdCStringSynthProvider(StringLikeSynthProvider):
    def _update(self):
        inner = gcm(self.valobj, 'inner')
        self.len = gcm(inner, 'length').GetValueAsUnsigned() - 1
        self.ptr = gcm(inner, 'data_ptr')

class StdOsStringSynthProvider(StringLikeSynthProvider):
    def _update(self):
        vec = gcm(self.valobj, 'inner', 'inner')
        self.len = gcm(vec, 'len').GetValueAsUnsigned()
        self.ptr = gcm(vec, 'buf', 'ptr', 'pointer', '__0')
