use crate::prelude::*;

use super::*;
use adapter_protocol::*;
use lldb::*;

impl super::DebugSession {
    pub(super) fn handle_disassemble(&mut self, args: DisassembleArguments) -> Result<DisassembleResponseBody, Error> {
        fn invalid_instruction() -> DisassembledInstruction {
            DisassembledInstruction {
                address: "0".into(),
                instruction: "<invalid>".into(),
                ..Default::default()
            }
        }

        let base_addr = parse_int::parse::<u64>(&args.memory_reference)?;
        let base_addr = match args.offset {
            Some(offset) => base_addr.wrapping_add(offset as u64),
            None => base_addr,
        };
        let instruction_offset = args.instruction_offset.unwrap_or(0);
        if args.instruction_count < 0 {
            bail!("Invalid instruction_count");
        }
        let instruction_count = args.instruction_count as usize;
        let resolve_symbols = args.resolve_symbols.unwrap_or(true);

        let flavor = self.target.disassembly_flavor();
        let mut result = if instruction_offset >= 0 {
            // Disassemble forward from the base address
            let start_addr = SBAddress::from_load_address(base_addr, &self.target);
            let instructions = self.target.read_instructions(
                &start_addr,
                (instruction_offset + args.instruction_count) as u32,
                flavor.as_deref(),
            );
            let mut dis_instructions = Vec::new();
            for instr in instructions.iter().skip(instruction_offset as usize) {
                dis_instructions.push(sbinstr_to_disinstr(&instr, &self.target, resolve_symbols, |fs| {
                    self.map_filespec_to_local(fs)
                }));
            }
            dis_instructions
        } else {
            // Disassemble backward from the base address
            let bytes_per_instruction = max_instruction_bytes(&self.target);
            let offset_bytes = -instruction_offset * bytes_per_instruction as i64;
            let start_addr = base_addr.wrapping_sub(offset_bytes as u64);
            let mut disassemble_bytes = instruction_count * bytes_per_instruction as usize;

            let mut dis_instructions = Vec::new();

            let expected_index = -instruction_offset as usize;

            // We make sure to extend disassemble_bytes to ensure that base_addr is always included
            if start_addr + (disassemble_bytes as u64) < base_addr {
                disassemble_bytes = (base_addr - start_addr + bytes_per_instruction) as usize;
            }

            for shuffle_count in 0..bytes_per_instruction {
                let instructions =
                    disassemble_byte_range(start_addr - shuffle_count, disassemble_bytes, &self.target.process())?;
                // Find the entry for the requested instruction. If it exists (i.e. there is a valid instruction
                // with the requested base address, then we're done and just need to splice the result array
                // to match the required output. Otherwise, move back a byte and try again.
                if let Some(index) =
                    instructions.iter().position(|i| i.address().load_address(&self.target) == base_addr)
                {
                    // Found it. Convert to the DAP instruction representation.
                    for instr in &instructions {
                        dis_instructions.push(sbinstr_to_disinstr(instr, &self.target, resolve_symbols, |fs| {
                            self.map_filespec_to_local(fs)
                        }));
                    }

                    // We need to make sure that the entry for the requested address, is precicely at
                    // the index expected, i.e. -instruction_offset
                    if index < expected_index {
                        // Pad the start with expected_index - index dummy instructions
                        dis_instructions.splice(
                            0..0,
                            std::iter::repeat_with(invalid_instruction).take(expected_index - index),
                        );
                    } else if index > expected_index {
                        let new_first = index - expected_index;
                        dis_instructions = dis_instructions.split_off(new_first);
                    }

                    // Confirm that we have the requested instruction at the correct location.
                    // We have to parse the address, but it's only in an assertion/debug build.
                    assert!(
                        dis_instructions.len() > expected_index
                            && parse_int::parse::<u64>(&dis_instructions[expected_index].address).unwrap() == base_addr
                    );
                    break;
                }
            }
            dis_instructions
        };

        // Ensure we have _exactly_ instruction_count elements
        result.resize_with(instruction_count, invalid_instruction);
        result.truncate(instruction_count);

        Ok(DisassembleResponseBody { instructions: result })
    }
}

fn disassemble_byte_range(start: Address, count: usize, process: &SBProcess) -> Result<Vec<SBInstruction>, Error> {
    let target = process.target();
    let mut buffer = Vec::new();
    buffer.resize(count, 0);
    process.read_memory(start, &mut buffer)?;

    let mut dis_instructions = Vec::new();
    let mut idx = 0;
    while idx < buffer.len() {
        let base_addr = SBAddress::from_load_address(start, &target);
        let instructions = target.get_instructions(&base_addr, &buffer[idx..]);
        if instructions.len() == 0 {
            // We were unable to disassemble _anything_ from the requested address. It's _probably_ an invalid address.
            // Bail at this point. The caller must attempt to validate the result as best it can. (in practice, caller
            // looks for a specific instruction at a specific address and adjusts the output based on that).
            return Ok(dis_instructions);
        } else {
            for instr in instructions.iter() {
                idx += instr.byte_size();
                dis_instructions.push(instr);
            }
        }
    }
    Ok(dis_instructions)
}

fn sbinstr_to_disinstr<F>(
    instr: &SBInstruction,
    target: &SBTarget,
    resolve_symbols: bool,
    map_filespec_to_local: F,
) -> DisassembledInstruction
where
    F: FnOnce(&SBFileSpec) -> Option<Rc<PathBuf>>,
{
    let mnemonic = instr.mnemonic(target);
    let operands = instr.operands(target);
    let comment = instr.comment(target);
    let comment_sep = if comment.is_empty() { "" } else { "  ; " };
    let address = instr.address();
    let instruction_str = format!("{:<6} {}{}{}", mnemonic, operands, comment_sep, comment);
    let mut dis_instr = DisassembledInstruction {
        address: format!("0x{:X}", address.load_address(target)),
        instruction: instruction_str,
        ..Default::default()
    };
    let mut instr_bytes = Vec::new();
    instr_bytes.resize(instr.byte_size(), 0);
    if instr.data(target).read_raw_data(0, &mut instr_bytes).is_ok() {
        let mut bytes_str = String::with_capacity(instr_bytes.len() * 3);
        for b in instr_bytes.iter() {
            let _ = write!(bytes_str, "{:02X} ", b);
        }
        dis_instr.instruction_bytes = Some(bytes_str);
    }

    if let Some(line_entry) = address.line_entry() {
        dis_instr.location = Some(Source {
            name: Some(line_entry.file_spec().filename().display().to_string()),
            path: match map_filespec_to_local(&line_entry.file_spec()) {
                Some(local_path) => Some(local_path.display().to_string()),
                _ => None,
            },
            ..Default::default()
        });
        dis_instr.line = Some(line_entry.line() as i64);
        dis_instr.column = Some(line_entry.column() as i64);
    }

    if resolve_symbols {
        if let Some(symbol) = instr.address().symbol() {
            dis_instr.symbol = Some(symbol.name().into());
        }
    }
    dis_instr
}

fn max_instruction_bytes(target: &SBTarget) -> u64 {
    let arch = target.triple().split("-").next().unwrap();
    if arch.starts_with("arm") || arch.starts_with("aarch64") || arch.starts_with("riscv") {
        return 4;
    }
    return 16;
}
