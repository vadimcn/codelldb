import logging
import lldb
from . import handles

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

class AddressSpace:
    def __init__(self, target):
        self.target = target
        self.by_handle = handles.Handles()
        self.by_address = [] # A list of DisassembledRange's sorted by start_address

    def create_from_address(self, addr):
        symbol = addr.GetSymbol()
        if symbol.IsValid():
            start_addr = symbol.GetStartAddress()
            end_addr = symbol.GetEndAddress()
            instructions = symbol.GetInstructions(self.target)
        else:
            start_addr = addr
            instructions = self.target.ReadInstructions(start_addr, NO_SYMBOL_INSTRUCTIONS + 1)
            if len(instructions) == 0:
                return None
            last_instr = instructions[len(instructions)-1] # SBInstructionList doesn't support negative indices!
            end_addr = last_instr.GetAddress()
        dasm = DisassembledRange(self.target, start_addr, end_addr, instructions)
        self.insert(dasm)
        return dasm

    def create_from_range(self, start_addr, end_addr):
        error = lldb.SBError()
        length = end_addr.GetLoadAddress(self.target) - start_addr.GetLoadAddress(self.target)
        instr_bytes = self.target.ReadMemory(start_addr, length, error)
        instructions = self.target.GetInstructions(start_addr, instr_bytes)
        dasm = DisassembledRange(self.target, start_addr, end_addr, instructions)
        self.insert(dasm)
        return dasm

    def get_by_handle(self, h):
        return self.by_handle.get(h)

    # Find a Dissassembly whose range the address belongs to (assuming a is sorted on start_address)
    def get_by_address(self, address):
        if isinstance(address, lldb.SBAddress):
            address = address.GetLoadAddress(self.target)
        a = self.by_address
        i  = upper_bound(a, address, lambda dasm: dasm.start_address) - 1
        if i >= 0 and a[i].start_address <= address < a[i].end_address:
            return a[i]
        else:
            return None

    def insert(self, dasm):
        log.info('Adding disassembled range 0x%x-0x%x', dasm.start_address, dasm.end_address)
        a = self.by_address
        i = lower_bound(a, dasm.start_address, lambda dasm: dasm.start_address)
        assert i == len(a) or dasm.start_address != a[i].start_address
        a.insert(i, dasm)
        dasm.source_ref = self.by_handle.create(dasm)

class DisassembledRange:
    start_sbaddr = None # SBAddress
    start_address = None # physical address
    end_address = None # physical address
    target = None
    source_ref = None

    def __init__(self, target, start_sbaddr, end_sbaddr, instructions):
        self.target = target
        self.start_sbaddr = start_sbaddr
        self.end_sbaddr = end_sbaddr
        self.start_address = start_sbaddr.GetLoadAddress(target)
        self.end_address = end_sbaddr.GetLoadAddress(target)
        self.instructions = instructions
        self.source_name = "@%x..%x" % (self.start_address, self.end_address)
        self.line_addresses = [-1, -1] # addresses corresponding to source lines (-1 = comment)
        for instr in self.instructions:
            self.line_addresses.append(instr.GetAddress().GetLoadAddress(self.target))

    def line_num_by_address(self, load_addr):
        return lower_bound(self.line_addresses, load_addr) + 1 # lines numbers are 1-based

    def address_by_line_num(self, line_num):
        return self.line_addresses[line_num - 1]

    def get_source_text(self):
        line_entry = self.start_sbaddr.GetLineEntry()
        if line_entry.IsValid():
            source_location = '%s:%d' % (line_entry.GetFileSpec(), line_entry.GetLine())
        else:
            source_location = 'unknown'
        symbol = self.start_sbaddr.GetSymbol()
        if symbol.IsValid():
            desc = lldb.SBStream()
            symbol.GetDescription(desc)
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
