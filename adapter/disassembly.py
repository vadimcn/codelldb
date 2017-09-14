import logging
import lldb

log = logging.getLogger('disassembly')

MAX_INSTR_BYTES = 8 # Max number of instruction bytes to show.
NO_SYMBOL_INSTRUCTIONS = 32 # How many instructions to show when there isn't a symbol associated
                            # with the PC location.

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
    else:
        return None

# Insert Dissassembly in sorted order
def insert(a, dasm):
    i = lower_bound(a, dasm.start_address, lambda dasm: dasm.start_address)
    assert i == len(a) or dasm.start_address != a[i].start_address
    a.insert(i, dasm)

class Disassembly:
    start_sbaddr = None # SBAddress
    start_address = None # physical address
    end_address = None # physical address
    target = None
    source_ref = None

    def __init__(self, pc_sbaddr, target):
        self.target = target
        self.symbol = symbol = pc_sbaddr.GetSymbol()
        pc_address = pc_sbaddr.GetLoadAddress(self.target)
        if symbol.IsValid():
            self.start_sbaddr = symbol.GetStartAddress()
            self.start_address = self.start_sbaddr.GetLoadAddress(self.target)
            self.source_name = '%s @%x' % (symbol.GetName(), self.start_address)
            self.instructions = symbol.GetInstructions(self.target)
            if len(self.instructions):
                last_instr = self.instructions[len(self.instructions)-1]
                self.end_address = last_instr.GetAddress().GetLoadAddress(self.target) + last_instr.GetByteSize()

        if not symbol.IsValid() or not len(self.instructions) or \
                not (self.start_address <= pc_address < self.end_address):
            # Just read some instructions around the PC location.
            self.start_sbaddr = pc_sbaddr
            self.start_address = pc_sbaddr.GetLoadAddress(self.target)
            self.source_name = "@%x" % self.start_address
            self.instructions = self.target.ReadInstructions(pc_sbaddr, NO_SYMBOL_INSTRUCTIONS)
            if len(self.instructions):
                last_instr = self.instructions[len(self.instructions)-1]
                self.end_address = last_instr.GetAddress().GetLoadAddress(self.target) + last_instr.GetByteSize()
                assert self.start_address <= pc_address < self.end_address
            else:
                self.end_address = self.start_address

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
        line_entry = self.start_sbaddr.GetLineEntry()
        if line_entry.IsValid():
            source_location = '%s:%d' % (line_entry.GetFileSpec(), line_entry.GetLine())
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
