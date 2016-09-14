import logging
import lldb
import bisect

log = logging.getLogger('disassembly')

class Disassembly:
    def __init__(self, symbol, line_entry, target):
        self.symbol = symbol
        self.line_entry = line_entry
        self.target = target
        self.start_address = symbol.GetStartAddress().GetLoadAddress(self.target)
        self.addresses = [-1, -1] # addresses corresponding to source lines (-1 = comment)
        for instr in self.symbol.GetInstructions(self.target):
            self.addresses.append(instr.GetAddress().GetLoadAddress(self.target))

    def line_num_by_address(self, load_addr):
        return bisect.bisect_left(self.addresses, load_addr) + 1 # lines numbers are 1-based

    def address_by_line_num(self, line_num):
        return self.addresses[line_num - 1]

    def get_address(self):
        return self.start_address

    def get_source_ref(self):
        source_name = '%s @%x' % (self.symbol.GetName(), self.start_address)
        return { 'name': source_name, 'sourceReference': self.source_ref }

    def get_source_text(self):
        source_location = '<unknown>'
        if self.line_entry.IsValid():
            source_location = '%s:%d' % (self.line_entry.GetFileSpec(), self.line_entry.GetLine())
        desc = lldb.SBStream()
        self.symbol.GetDescription(desc)
        lines = [
            '; %s' % (desc.GetData()),
            '; Source location: %s' % source_location ]
        for instr in self.symbol.GetInstructions(self.target):
            addr = instr.GetAddress().GetLoadAddress(self.target)
            dump = ''
            for b in instr.GetData(self.target).uint8:
                dump += '%02X ' % b
            line = '%08X: %-25s %-6s %s' % (addr, dump,
                instr.GetMnemonic(self.target), instr.GetOperands(self.target))
            comment = instr.GetComment(self.target)
            if len(comment) > 0:
                line += '  ; ' + comment
            #line = str(instr)
            lines.append(line)
        return '\n'.join(lines)
