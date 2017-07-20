import logging
import lldb
import codecs
import re
from .. import xrange, to_lldb_str

log = logging.getLogger('rust')

rust_category = None
analyze_value = None

def initialize(debugger, analyze):
    log.info('initialize')
    global rust_category
    global analyze_value
    analyze_value = analyze

    debugger.HandleCommand('script import adapter.formatters.rust')
    rust_category = debugger.CreateCategory('Rust')
    #rust_category.AddLanguage(lldb.eLanguageTypeRust)
    rust_category.SetEnabled(True)

    attach_summary_to_type('get_str_slice_summary', '&str')
    attach_synthetic_to_type('SliceSynthProvider', '&str')

    attach_summary_to_type('get_string_summary', 'collections::string::String')
    attach_synthetic_to_type('StdStringSynthProvider', 'collections::string::String')
    # New String location
    attach_summary_to_type('get_string_summary', 'alloc::string::String')
    attach_synthetic_to_type('StdStringSynthProvider', 'alloc::string::String')

    attach_summary_to_type('get_array_summary', r'^.*\[[0-9]+\]$', True)

    attach_summary_to_type('get_vector_summary', r'^collections::vec::Vec<.+>$', True)
    attach_synthetic_to_type('StdVectorSynthProvider', r'^collections::vec::Vec<.+>$', True)
    # New Vec location
    attach_summary_to_type('get_vector_summary', r'^alloc::vec::Vec<.+>$', True)
    attach_synthetic_to_type('StdVectorSynthProvider', r'^alloc::vec::Vec<.+>$', True)

    attach_summary_to_type('get_slice_summary', r'^&(mut\s*)?\[.*\]$', True)
    attach_synthetic_to_type('SliceSynthProvider', r'^&(mut\s*)?\[.*\]$', True)

    attach_summary_to_type('get_cstring_summary', 'std::ffi::c_str::CString')
    attach_synthetic_to_type('StdCStringSynthProvider', 'std::ffi::c_str::CString')

    attach_summary_to_type('get_osstring_summary', 'std::ffi::os_str::OsString')
    attach_synthetic_to_type('StdOsStringSynthProvider', 'std::ffi::os_str::OsString')

# Enums and tuples cannot be recognized based on type name.
# These require deeper runtime analysis to tease them apart.
ENUM_DISCRIMINANT = 'RUST$ENUM$DISR'
ENCODED_ENUM_PREFIX = 'RUST$ENCODED$ENUM$'

def analyze(sbvalue):
    #log.info('rust.analyze for %s %d', sbvalue.GetType().GetName(), sbvalue.GetType().GetTypeClass())
    obj_type = sbvalue.GetType().GetUnqualifiedType()
    type_class = obj_type.GetTypeClass()
    if type_class == lldb.eTypeClassUnion:
        num_fields = obj_type.GetNumberOfFields()
        if num_fields == 1:
            first_variant_name = obj_type.GetFieldAtIndex(0).GetName()
            if first_variant_name is None:
                # Singleton
                attach_summary_to_type('get_singleton_enum_summary', obj_type.GetName())
            else:
                assert first_variant_name.startswith(ENCODED_ENUM_PREFIX)
                attach_summary_to_type('get_encoded_enum_summary', obj_type.GetName())
                attach_synthetic_to_type('EncodedEnumProvider', obj_type.GetName())
        else:
            attach_summary_to_type('get_regular_enum_summary', obj_type.GetName())
            attach_synthetic_to_type('RegularEnumProvider', obj_type.GetName())
    elif type_class == lldb.eTypeClassStruct:
        if obj_type.GetFieldAtIndex(0).GetName() == ENUM_DISCRIMINANT:
            attach_summary_to_type('get_enum_variant_summary', obj_type.GetName())
        elif obj_type.GetFieldAtIndex(0).GetName() == '__0':
            attach_summary_to_type('get_tuple_summary', obj_type.GetName())

def attach_summary_to_type(summary_fn, type_name, is_regex=False):
    global rust_category
    summary = lldb.SBTypeSummary.CreateWithFunctionName('adapter.formatters.rust.' + summary_fn)
    summary.SetOptions(lldb.eTypeOptionCascade)
    assert rust_category.AddTypeSummary(lldb.SBTypeNameSpecifier(type_name, is_regex), summary)

def attach_synthetic_to_type(synth_class, type_name, is_regex=False):
    global rust_category
    synth = lldb.SBTypeSynthetic.CreateWithClassName('adapter.formatters.rust.' + synth_class)
    synth.SetOptions(lldb.eTypeOptionCascade)
    assert rust_category.AddTypeSynthetic(lldb.SBTypeNameSpecifier(type_name, is_regex), synth)

# Chained GetChildMemberWithName lookups
def gcm(valobj, *chain):
    for name in chain:
        valobj = valobj.GetChildMemberWithName(name)
    return valobj

def string_from_ptr(pointer, length):
    if length <= 0:
        return u''
    error = lldb.SBError()
    process = pointer.GetProcess()
    data = process.ReadMemory(pointer.GetValueAsUnsigned(), length, error)
    if error.Success():
        return data.decode('utf8', 'replace')
    else:
        log.error('%s', error.GetCString())

def get_obj_summary(valobj):
    analyze_value(valobj)
    summary = valobj.GetSummary()
    if summary is not None:
        return summary
    summary = valobj.GetValue()
    if summary is not None:
        return summary
    return '<not available>'

# 'get_summary' is annoyingly not a part of the standard LLDB synth provider API.
# This trick allows us to share data extraction logic between synth providers and their
# sibling summary providers.
def get_synth_summary(synth_class, valobj, dict):
    synth = synth_class(valobj.GetNonSyntheticValue(), dict)
    synth.update()
    summary = synth.get_summary()
    return to_lldb_str(summary)

def print_array_elements(valobj, maxsize=32):
    s = ''
    for i in xrange(0, valobj.GetNumChildren()):
        if len(s) > 0: s += ', '
        summary = get_obj_summary(valobj.GetChildAtIndex(i))
        s += summary if summary is not None else '?'
        if len(s) > maxsize:
            s += ' ...'
            break
    return s

def get_singleton_enum_summary(valobj, dict):
    return get_obj_summary(valobj.GetChildAtIndex(0))

def get_encoded_enum_summary(valobj, dict):
    return get_synth_summary(EncodedEnumProvider, valobj, dict)

def get_regular_enum_summary(valobj, doct):
    return get_synth_summary(RegularEnumProvider, valobj, dict)

def get_enum_variant_summary(valobj, dict):
    obj_type = valobj.GetType()
    num_fields = obj_type.GetNumberOfFields()
    unqual_type_name = get_unqualified_type_name(obj_type.GetName())
    if num_fields == 1:
        return unqual_type_name
    elif obj_type.GetFieldAtIndex(1).GetName().startswith('__'): # tuple variant
        fields = ', '.join([get_obj_summary(valobj.GetChildAtIndex(i)) for i in range(1, num_fields)])
        return '%s(%s)' % (unqual_type_name, fields)
    else: # struct variant
        fields = [valobj.GetChildAtIndex(i) for i in range(1, num_fields)]
        fields = ', '.join(['%s:%s' % (f.GetName(), get_obj_summary(f)) for f in fields])
        return '%s{%s}' % (unqual_type_name, fields)

def get_tuple_summary(valobj, dict):
    fields = [get_obj_summary(valobj.GetChildAtIndex(i)) for i in range(0, valobj.GetNumChildren())]
    return '(%s)' % ', '.join(fields)

def get_str_slice_summary(valobj, dict):
    return get_synth_summary(StrSliceSynthProvider, valobj, dict)

def get_string_summary(valobj, dict):
    return get_synth_summary(StdStringSynthProvider, valobj, dict)

def get_cstring_summary(valobj, dict):
    return get_synth_summary(StdCStringSynthProvider, valobj, dict)

def get_osstring_summary(valobj, dict):
    return get_synth_summary(StdOsStringSynthProvider, valobj, dict)

def get_array_summary(valobj, dict):
    return '(%d) [%s]' % (valobj.GetNumChildren(), print_array_elements(valobj))

def get_vector_summary(valobj, dict):
    length = valobj.GetNumChildren()
    return '(%d) vec![%s]' % (length, print_array_elements(valobj))

def get_slice_summary(valobj, dict):
    length = valobj.GetNumChildren()
    return '(%d) &[%s]' % (length, print_array_elements(valobj))

UNQUAL_TYPE_MARKERS = ["(", "[", "&", "*"]
UNQUAL_TYPE_REGEX = re.compile(r'^(?:\w+::)*(\w+).*', re.UNICODE)
def get_unqualified_type_name(type_name):
    if type_name[0] in UNQUAL_TYPE_MARKERS:
        return type_name
    return UNQUAL_TYPE_REGEX.match(type_name).group(1)

# LLDB is somewhat unpredictable about when it calls update() on synth providers.
# Don't want to put `if self.is_initalized: self.update()` in each method, so...
class RustSynthProvider:
    def __init__(self, valobj, dict={}):
        self.valobj = valobj
        self._real_class = self.__class__
        self.__class__ = RustSynthProvider.Lazy
    # default impls
    def initialize(self): return None
    def update(self): return None
    def num_children(self): return 0
    def has_children(self): return False
    def get_child_at_index(self, index): return None
    def get_child_index(self, name): return None
    def get_summary(self): return None

    class Lazy:
        def _do_init(self):
            self.__class__ = self._real_class
            self.initialize() # first-time initialization
            self.update()
            return self
        def update(self):
            self._do_init()
            # _do_init() already called update()
        def num_children(self):
            return self._do_init().num_children()
        def has_children(self):
            return self._do_init().has_children()
        def get_child_at_index(self, index):
            return self._do_init().get_child_at_index(index)
        def get_child_index(self, name):
            return self._do_init().get_child_index(name)
        def get_summary(self):
            return self._do_init().get_summary()

class EncodedEnumProvider(RustSynthProvider):
    def initialize(self):
        # 'Encoded' enums always have two variants, of which one contains no data,
        # and the other one contains a field (not necessarily at the top level) that implements
        # Zeroable.  This field is then used as a two-state discriminant.
        variant_name = self.valobj.GetType().GetFieldAtIndex(0).GetName()
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
            return '%s%s' % (unqual_type_name, get_obj_summary(self.variant))

class RegularEnumProvider(RustSynthProvider):
    def update(self):
        # Regular enums are represented as unions of structs, containing discriminant in the
        # first field.
        discriminant = self.valobj.GetChildAtIndex(0).GetChildAtIndex(0).GetValueAsUnsigned()
        self.variant = self.valobj.GetChildAtIndex(discriminant)

    def num_children(self):
        return max(0, self.variant.GetNumChildren() - 1)

    def has_children(self):
        return self.num_children() > 0

    def get_child_at_index(self, index):
        return self.variant.GetChildAtIndex(index + 1)

    def get_child_index(self, name):
        return self.variant.GetIndexOfChildWithName(name) - 1

    def get_summary(self):
        return get_obj_summary(self.variant)

# Base class for providers that represent array-like objects
class ArrayLikeSynthProvider(RustSynthProvider):
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
        # Limit string length to 1000 characters to cope with uninitialized values whose
        # length field contains garbage.
        strval = string_from_ptr(self.ptr, min(self.len, 1000))
        if strval == None:
            return None
        if self.len > 1000: strval += u'...'
        return u'"%s"' % strval

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
        tmp = gcm(vec, 'bytes') # Windows OSString has an extra layer
        if tmp.IsValid():
            vec = tmp
        self.len = gcm(vec, 'len').GetValueAsUnsigned()
        self.ptr = gcm(vec, 'buf', 'ptr', 'pointer', '__0')
