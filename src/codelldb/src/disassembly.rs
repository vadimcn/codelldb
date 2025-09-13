use crate::prelude::*;
use serde_derive::*;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::ops::Range;
use std::rc::Rc;
use std::str;

use crate::handles::Handle;
use lldb::*;

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
        let idx = ranges.by_address.partition_point(|dasm| dasm.load_range.start <= load_addr);
        if idx > 0 {
            let dasm = &ranges.by_address[idx - 1];
            if dasm.load_range.contains(&load_addr) {
                return Some(dasm.clone());
            }
        }
        None
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
            load_range: start_load_addr..end_load_addr,
            source_name: source_name,
            instructions: instructions,
            instruction_addresses: instruction_addrs,
        });
        ranges.by_handle.insert(handle, dasm.clone());
        let idx = ranges.by_address.partition_point(|dasm| dasm.load_range.start < start_load_addr);
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
    load_range: Range<Address>,
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
        self.instruction_addresses.partition_point(|addr| *addr < load_addr) as u32 + 3
    }

    pub fn address_by_line_num(&self, line: u32) -> Address {
        self.instruction_addresses[line as usize - 3]
    }

    pub fn adapter_data(&self) -> AdapterData {
        let line_offsets = self.instruction_addresses.windows(2).map(|w| (w[1] - w[0]) as u32).collect();
        AdapterData {
            start: self.load_range.start,
            end: self.load_range.end,
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

    pub fn get_source_text(&self, max_instr_bytes: usize) -> String {
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

        let mut instr_data = vec![];
        let mut dump = String::new();
        for instr in self.instructions.iter() {
            let load_addr = instr.address().load_address(&self.target);
            instr_data.resize(instr.byte_size(), 0);
            instr.data(&self.target).read_raw_data(0, &mut instr_data).unwrap();
            dump.clear();
            for (i, b) in instr_data.iter().enumerate() {
                if i >= max_instr_bytes {
                    dump.push('>');
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
                dumpwidth=max_instr_bytes * 3 + 2
            );
        }

        text
    }
}

#[test]
fn test_range_lookup() {
    use crate::TEST_DEBUGGER;
    use std::rc::Rc;
    let target = TEST_DEBUGGER.dummy_target();
    let ranges = DisassembledRanges::new(&target);

    assert!(ranges.find_by_address(1234).is_none());

    let add = |start, end| {
        let start = SBAddress::from_load_address(start, &target);
        let end = SBAddress::from_load_address(end, &target);
        ranges.add(start.clone(), end, target.get_instructions(&start, &[], None))
    };

    let dasm1 = add(1000, 2000);
    let dasm3 = add(4000, 5000);
    let dasm2 = add(3000, 4000);

    assert!(Rc::ptr_eq(&ranges.find_by_address(1000).unwrap(), &dasm1));
    assert!(Rc::ptr_eq(&ranges.find_by_address(1234).unwrap(), &dasm1));
    assert!(Rc::ptr_eq(&ranges.find_by_address(1999).unwrap(), &dasm1));
    assert!(ranges.find_by_address(2000).is_none());
    assert!(Rc::ptr_eq(&ranges.find_by_address(3000).unwrap(), &dasm2));
    assert!(Rc::ptr_eq(&ranges.find_by_address(3999).unwrap(), &dasm2));
    assert!(Rc::ptr_eq(&ranges.find_by_address(4000).unwrap(), &dasm3));
    assert!(ranges.find_by_address(5000).is_none());
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
