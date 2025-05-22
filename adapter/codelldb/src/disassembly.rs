use crate::prelude::*;
use serde_derive::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;
use std::str;

use crate::handles::Handle;
use lldb::*;
use superslice::Ext;

struct Ranges {
    pub by_handle: HashMap<Handle, Rc<DisassembledRange>>,
    pub by_address: Vec<Rc<DisassembledRange>>,
}
pub struct DisassembledRanges {
    target: SBTarget,
    ranges: RefCell<Ranges>,
}

impl DisassembledRanges {
    pub fn new(target: &SBTarget) -> DisassembledRanges {
        DisassembledRanges {
            target: target.clone(),
            ranges: RefCell::new(Ranges {
                by_handle: HashMap::new(),
                by_address: Vec::new(),
            }),
        }
    }

    pub fn find_by_handle(&self, handle: Handle) -> Option<Rc<DisassembledRange>> {
        self.ranges.borrow_mut().by_handle.get(&handle).map(|dasm| dasm.clone())
    }

    fn find_by_address(&self, load_addr: Address) -> Option<Rc<DisassembledRange>> {
        let ranges = self.ranges.borrow_mut();
        let idx = ranges.by_address.upper_bound_by_key(&load_addr, |dasm| dasm.start_load_addr);
        if idx == 0 {
            None
        } else {
            let dasm = &ranges.by_address[idx - 1];
            if dasm.start_load_addr <= load_addr && load_addr < dasm.end_load_addr {
                Some(dasm.clone())
            } else {
                None
            }
        }
    }

    pub fn from_address(&self, load_addr: Address) -> Result<Rc<DisassembledRange>, Error> {
        if let Some(dasm) = self.find_by_address(load_addr) {
            return Ok(dasm);
        }

        let addr = SBAddress::from_load_address(load_addr, &self.target);
        debug!("{:?}", addr);

        let start_addr;
        let end_addr;
        let instructions;
        match addr.symbol() {
            Some(symbol) => {
                start_addr = symbol.start_address();
                end_addr = symbol.end_address();
                instructions = symbol.instructions(&self.target);
            }
            None => {
                // How many instructions to put into DisassembledRange if the address is not in scope of any symbol.
                const NO_SYMBOL_INSTRUCTIONS: u32 = 32;
                start_addr = addr.clone();
                let flavor = self.target.disassembly_flavor();
                instructions =
                    self.target.read_instructions(&start_addr, NO_SYMBOL_INSTRUCTIONS + 1, flavor.as_deref());
                end_addr = if instructions.len() > 0 {
                    let last_instr = instructions.instruction_at_index((instructions.len() - 1) as u32);
                    last_instr.address()
                } else {
                    start_addr.clone()
                };
            }
        }

        if instructions.len() > 0 {
            Ok(self.add(start_addr, end_addr, instructions))
        } else {
            bail!("Can't read instructions at that address.")
        }
    }

    fn add(
        &self,
        start_addr: SBAddress,
        end_addr: SBAddress,
        instructions: SBInstructionList,
    ) -> Rc<DisassembledRange> {
        let mut ranges = self.ranges.borrow_mut();
        let handle = Handle::new((ranges.by_handle.len() + 1000) as u32).unwrap();
        let instruction_addrs = instructions.iter().map(|i| i.address().load_address(&self.target)).collect();
        let start_load_addr = start_addr.load_address(&self.target);
        let end_load_addr = end_addr.load_address(&self.target);
        let source_name = match start_addr.symbol() {
            Some(symbol) => format!("@{}", symbol.name()),
            None => format!("@{:x}..{:x}", start_load_addr, end_load_addr),
        };
        let dasm = Rc::new(DisassembledRange {
            handle: handle,
            target: self.target.clone(),
            start_addr: start_addr,
            start_load_addr: start_load_addr,
            end_load_addr: end_load_addr,
            source_name: source_name,
            instructions: instructions,
            instruction_addresses: instruction_addrs,
        });
        ranges.by_handle.insert(handle, dasm.clone());
        let idx = ranges.by_address.lower_bound_by_key(&dasm.start_load_addr, |dasm| dasm.start_load_addr);
        ranges.by_address.insert(idx, dasm.clone());
        dasm
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct AdapterData {
    pub start: Address,
    pub end: Address,
    pub line_offsets: Vec<u32>,
}

#[derive(Debug)]
pub struct DisassembledRange {
    handle: Handle,
    target: SBTarget,
    start_addr: SBAddress,
    start_load_addr: Address,
    end_load_addr: Address,
    source_name: String,
    instructions: SBInstructionList,
    instruction_addresses: Vec<Address>,
}

impl DisassembledRange {
    pub fn handle(&self) -> Handle {
        self.handle
    }

    pub fn source_name(&self) -> &str {
        &self.source_name
    }

    pub fn line_num_by_address(&self, load_addr: Address) -> u32 {
        self.instruction_addresses.lower_bound(&load_addr) as u32 + 3
    }

    pub fn address_by_line_num(&self, line: u32) -> Address {
        self.instruction_addresses[line as usize - 3]
    }

    pub fn adapter_data(&self) -> AdapterData {
        let line_offsets = self.instruction_addresses.windows(2).map(|w| (w[1] - w[0]) as u32).collect();
        AdapterData {
            start: self.start_load_addr,
            end: self.end_load_addr,
            line_offsets: line_offsets,
        }
    }

    pub fn lines_from_adapter_data(adapter_data: &AdapterData) -> Vec<Address> {
        std::iter::repeat(adapter_data.start)
            .take(4) // lines are 1-based + 2 header lines + 1st instruction
            .chain(
                adapter_data.line_offsets.iter().scan(adapter_data.start, |addr, &delta| {
                    *addr += delta as u64;
                    Some(*addr)
                }),
            )
            .collect()
    }

    pub fn get_source_text(&self) -> String {
        let mut text = String::new();

        #[allow(unused_must_use)]
        {
            write!(text, "; Symbol: ");
            if let Some(symbol) = self.start_addr.symbol() {
                write!(text, "{}", symbol.display_name());
                let mangled = symbol.mangled_name();
                if mangled.len() > 0 {
                    write!(text, ", mangled name={}", mangled);
                }
            } else {
                write!(text, "no symbol info");
            }
            writeln!(text, "");

            write!(text, "; Source: ");
            if let Some(le) = self.start_addr.line_entry() {
                write!(text, "{}:{}", le.file_spec().path().display(), le.line());
            } else {
                write!(text, "unknown");
            }
            writeln!(text, "");
        }

        const MAX_INSTR_BYTES: usize = 8;
        let mut instr_data = vec![];
        let mut dump = String::new();
        for instr in self.instructions.iter() {
            let load_addr = instr.address().load_address(&self.target);
            instr_data.resize(instr.byte_size(), 0);
            instr.data(&self.target).read_raw_data(0, &mut instr_data).unwrap();
            dump.clear();
            for (i, b) in instr_data.iter().enumerate() {
                if i >= MAX_INSTR_BYTES {
                    let _ = write!(dump, ">");
                    break;
                }
                let _ = write!(dump, "{:02X} ", b);
            }
            let mnemonic = instr.mnemonic(&self.target);
            let operands = instr.operands(&self.target);
            let comment = instr.comment(&self.target);
            let comment_sep = if comment.is_empty() { "" } else { "  ; " };
            #[rustfmt::skip]
            let _ = writeln!(text, "{:08X}: {:<dumpwidth$} {:<6} {}{}{}", //.
                load_addr, dump, mnemonic, operands, comment_sep, comment,
                dumpwidth=MAX_INSTR_BYTES * 3 + 2
            );
        }

        text
    }
}

#[test]
fn test_adapter_data() {
    let addresses = &[10, 20, 23, 25, 30, 35, 41, 42, 50];
    let adapter_data = AdapterData {
        start: 10,
        end: 55,
        line_offsets: addresses.windows(2).map(|w| (w[1] - w[0]) as u32).collect(),
    };
    let addresses2 = DisassembledRange::lines_from_adapter_data(&adapter_data);
    assert_eq!(addresses, &addresses2[3..]);
}
