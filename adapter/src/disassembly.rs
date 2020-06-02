use crate::prelude::*;
use serde_derive::*;
use std::borrow::Cow;
use std::cell::RefCell;
use std::collections::HashMap;
use std::fmt::Write;
use std::rc::Rc;
use std::str;

use crate::handles::Handle;
use lldb::*;
use superslice::Ext;

pub struct AddressSpace {
    target: SBTarget,
    by_handle: HashMap<Handle, Rc<DisassembledRange>>,
    by_address: Vec<Rc<DisassembledRange>>,
}

impl AddressSpace {
    pub fn new(target: &SBTarget) -> AddressSpace {
        AddressSpace {
            target: target.clone(),
            by_handle: HashMap::new(),
            by_address: Vec::new(),
        }
    }

    pub fn find_by_handle(&self, handle: Handle) -> Option<Rc<DisassembledRange>> {
        self.by_handle.get(&handle).map(|dasm| dasm.clone())
    }

    pub fn find_by_address(&self, load_addr: Address) -> Option<Rc<DisassembledRange>> {
        let idx = self.by_address.upper_bound_by_key(&load_addr, |dasm| dasm.start_load_addr);
        if idx == 0 {
            None
        } else {
            let dasm = &self.by_address[idx - 1];
            if dasm.start_load_addr <= load_addr && load_addr < dasm.end_load_addr {
                Some(dasm.clone())
            } else {
                None
            }
        }
    }

    pub fn get_by_address(&mut self, load_addr: Address) -> Rc<DisassembledRange> {
        if let Some(dasm) = self.find_by_address(load_addr) {
            return dasm;
        }

        let addr = SBAddress::from_load_address(load_addr, &self.target);
        assert!(addr.is_valid());
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
                instructions = self.target.read_instructions(&start_addr, NO_SYMBOL_INSTRUCTIONS + 1);
                end_addr = if instructions.len() > 0 {
                    let last_instr = instructions.instruction_at_index((instructions.len() - 1) as u32);
                    last_instr.address()
                } else {
                    start_addr.clone()
                };
            }
        }
        self.add(start_addr, end_addr, instructions)
    }

    fn add(
        &mut self,
        start_addr: SBAddress,
        end_addr: SBAddress,
        instructions: SBInstructionList,
    ) -> Rc<DisassembledRange> {
        let handle = Handle::new((self.by_handle.len() + 1000) as u32).unwrap();
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
            end_addr: end_addr,
            start_load_addr: start_load_addr,
            end_load_addr: end_load_addr,
            source_name: source_name,
            instructions: instructions,
            instruction_addresses: instruction_addrs,
            source_text: RefCell::new(None),
        });
        self.by_handle.insert(handle, dasm.clone());
        let idx = self.by_address.lower_bound_by_key(&dasm.start_load_addr, |dasm| dasm.start_load_addr);
        self.by_address.insert(idx, dasm.clone());
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
    end_addr: SBAddress,
    start_load_addr: Address,
    end_load_addr: Address,
    source_name: String,
    instructions: SBInstructionList,
    instruction_addresses: Vec<Address>,
    source_text: RefCell<Option<String>>,
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
        Some(adapter_data.start)
            .into_iter()
            .chain(adapter_data.line_offsets.iter().scan(adapter_data.start, |addr, &delta| {
                *addr += delta as u64;
                Some(*addr)
            }))
            .collect()
    }

    pub fn get_source_text(&self) -> String {
        let source_location: Cow<str> = match self.start_addr.line_entry() {
            Some(le) => format!("{}:{}", le.file_spec().path().display(), le.line()).into(),
            None => "unknown".into(),
        };

        let description: Cow<str> = match self.start_addr.symbol() {
            Some(symbol) => {
                let mut descr = SBStream::new();
                if symbol.get_description(&mut descr) {
                    match str::from_utf8(descr.data()) {
                        Ok(s) => Some(s.to_owned().into()),
                        Err(_) => None,
                    }
                } else {
                    None
                }
            }
            None => None,
        }
        .unwrap_or("No Symbol Info".into());

        let mut text = String::new();
        let _ = writeln!(text, "; {}", description);
        let _ = writeln!(text, "; Source location: {}", source_location);

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
            let comment_sep = if comment.is_empty() {
                ""
            } else {
                "  ; "
            };
            #[rustfmt::skip]
            let _ = writeln!(text, "{:08X}: {:<dumpwidth$} {:<6} {}{}{}", //.
                load_addr, dump, mnemonic, operands, comment_sep, comment,
                dumpwidth=MAX_INSTR_BYTES * 3 + 2
            );
        }

        text
    }
}
