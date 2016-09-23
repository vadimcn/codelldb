import logging
import lldb

log = logging.getLogger('disassembly')

MAX_INSTR_BYTES = 8 # Max number of instruction bytes to show

# bisect_left with get_key
def lower_bound(a, x, get_key = lambda x: x):
    lo = 0
    hi = len(a)
    while lo < hi:
        mid = (lo+hi)//2
        if get_key(a[mid]) < x: lo = mid+1
        else: hi = mid
    return lo

# bisect_right with get_key
def upper_bound(a, x, get_key = lambda x: x):
    lo = 0
    hi = len(a)
    while lo < hi:
        mid = (lo+hi)//2
        if x < get_key(a[mid]): hi = mid
        else: lo = mid+1
    return lo

# Find a Dissassembly whose range the address belongs to (assuming a is sorted on start_address)
def find(a, address):
    i  = upper_bound(a, address, lambda dasm: dasm.start_address) - 1
    if i >= 0 and a[i].start_address <= address < a[i].end_address:
        return a[i]
    return None

# Insert Dissassembly in sorted order
def insert(a, dasm):
    i = lower_bound(a, dasm.start_address, lambda dasm: dasm.start_address)
    assert i == len(a) or dasm.start_address != a[i].start_address
    a.insert(i, dasm)

class Disassembly:
    start_address = None
    end_address = None
    source_ref = None

    def __init__(self, frame, target):
        self.target = target
        symbol = frame.GetSymbol()
        self.symbol = symbol
        self.line_entry = frame.GetLineEntry()
        if symbol.IsValid():
            self.start_address = symbol.GetStartAddress().GetLoadAddress(self.target)
            self.source_name = '%s @%x' % (symbol.GetName(), self.start_address)
            self.instructions = symbol.GetInstructions(self.target)
        else:
            self.start_address = frame.GetPCAddress().GetLoadAddress(self.target)
            self.source_name = "@%x" % self.start_address
            self.instructions = self.target.ReadInstructions(frame.GetPCAddress(), 32)
        last_instr = self.instructions[len(self.instructions)-1]
        self.end_address = last_instr.GetAddress().GetLoadAddress(self.target) + last_instr.GetByteSize()
        self.addresses = [-1, -1] # addresses corresponding to source lines (-1 = comment)
        for instr in self.instructions:
            self.addresses.append(instr.GetAddress().GetLoadAddress(self.target))

    def line_num_by_address(self, load_addr):
        return lower_bound(self.addresses, load_addr) + 1 # lines numbers are 1-based

    def address_by_line_num(self, line_num):
        return self.addresses[line_num - 1]

    def get_source_ref(self):
        return { 'name': self.source_name, 'sourceReference': self.source_ref }

    def get_source_text(self):
        if self.line_entry.IsValid():
            source_location = '%s:%d' % (self.line_entry.GetFileSpec(), self.line_entry.GetLine())
        else:
            source_location = 'unknown'
        if self.symbol.IsValid():
            desc = lldb.SBStream()
            self.symbol.GetDescription(desc)
            description = desc.GetData()
        else:
            description = 'No symbol info'

        lines = [
            '; %s' % description,
            '; Source location: %s' % source_location ]
        for instr in self.instructions:
            addr = instr.GetAddress().GetLoadAddress(self.target)
            dump = ''
            for i,b in enumerate(instr.GetData(self.target).uint8):
                if i >= MAX_INSTR_BYTES:
                    dump += '>'
                    break
                dump += '%02X ' % b
            dump = dump.ljust(MAX_INSTR_BYTES * 3 + 2)
            line = '%08X: %s %-6s %s' % (addr, dump,
                instr.GetMnemonic(self.target), instr.GetOperands(self.target))
            comment = instr.GetComment(self.target)
            if len(comment) > 0:
                line += '  ; ' + comment
            #line = str(instr)
            lines.append(line)
        return '\n'.join(lines)
