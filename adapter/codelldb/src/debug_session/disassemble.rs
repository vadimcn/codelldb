use std::ops::Range;

use crate::prelude::*;

use super::*;
use adapter_protocol::*;
use lldb::*;

impl super::DebugSession {
    pub(super) fn handle_disassemble(&mut self, args: DisassembleArguments) -> Result<DisassembleResponseBody, Error> {
        let base_addr = parse_int::parse::<u64>(&args.memory_reference)?;
        let base_addr = match args.offset {
            Some(offset) => base_addr.wrapping_add(offset as u64),
            None => base_addr,
        };
        let instruction_offset = args.instruction_offset.unwrap_or(0);
        if args.instruction_count < 0 {
            bail!("Invalid instruction_count");
        }
        // We want to disassemble enough bytes so that:
        // 1. The instruction at base_addr is within the range, allowing us to check instruction stream alignment.
        // 2. The resulting list of instructions covers the requested range.
        let num_before = -instruction_offset.min(0) as u64;
        let num_after = (args.instruction_count + instruction_offset).max(0) as u64;
        let bytes_per_instruction = self.max_instruction_bytes();
        let start_addr = base_addr.saturating_sub(num_before * bytes_per_instruction);
        let end_addr = base_addr.saturating_add(num_after * bytes_per_instruction);

        let (mut start_addr, mut buffer) = self.read_memory(start_addr, end_addr - start_addr);
        let mut instructions = Vec::new();
        let mut base_index = None;
        for i in 0..bytes_per_instruction.min(buffer.len() as u64) {
            instructions = self.disassemble_bytes(start_addr + i, &buffer[i as usize..]);
            base_index = instructions.iter().position(|(ref arange, _)| arange.start == base_addr);
            if base_index.is_some() {
                // Adjust to reflect the actual range we've disassembled.
                start_addr += i;
                buffer.drain(..i as usize);
                break;
            }
        }
        let Some(base_index) = base_index else {
            bail!(format!("Failed to disassemble memory at 0x{base_addr:X}."))
        };
        let end_addr = start_addr + buffer.len() as u64;

        // Determine the range of decoded instructions to return.
        // We may end up with out-of-range indices, in which case we'll pad with "invalid memory" instructions.
        let mut start_idx = base_index as i64 + instruction_offset;
        let end_idx = start_idx + args.instruction_count;
        let mut dis_instructions = Vec::new();
        if start_idx < 0 {
            for i in start_idx..0 {
                let addr = start_addr.wrapping_add(i as u64);
                dis_instructions.push(invalid_memory(addr));
            }
            start_idx = 0;
        }
        let resolve_symbols = args.resolve_symbols.unwrap_or(true);
        let mut last_line_entry = None;
        for (arange, instr) in &instructions[start_idx as usize..end_idx as usize] {
            let dis_instsr = match instr {
                Ok(sbinstr) => {
                    let mnemonic = sbinstr.mnemonic(&self.target);
                    let operands = sbinstr.operands(&self.target);
                    let comment = sbinstr.comment(&self.target);
                    let comment_sep = if comment.is_empty() { "" } else { "  ; " };
                    let instruction_str = format!("{:<6} {}{}{}", mnemonic, operands, comment_sep, comment);

                    let instr_bytes = &buffer[(arange.start - start_addr) as usize..(arange.end - start_addr) as usize];
                    let mut bytes_str = String::with_capacity(instr_bytes.len() * 3);
                    for (i, b) in instr_bytes.iter().enumerate() {
                        if i >= self.max_instr_bytes {
                            bytes_str.push('>');
                            break;
                        }
                        let _ = write!(bytes_str, "{:02X} ", b);
                    }
                    let bytes_str = format!("{bytes_str:<width$}", width = self.max_instr_bytes * 3 + 2);

                    let mut dis_instr = DisassembledInstruction {
                        address: format!("0x{:X}", arange.start),
                        instruction_bytes: Some(bytes_str),
                        instruction: instruction_str,
                        ..Default::default()
                    };

                    let address = sbinstr.address();
                    let line_entry = address.line_entry();
                    if let Some(ref line_entry) = line_entry {
                        dis_instr.line = Some(line_entry.line() as i64);
                        dis_instr.column = Some(line_entry.column() as i64);
                        // Don't emit location unless different from the last one.
                        if last_line_entry.map_or(false, |lle| &lle != line_entry) {
                            dis_instr.location = Some(Source {
                                name: Some(line_entry.file_spec().filename().display().to_string()),
                                path: match self.map_filespec_to_local(&line_entry.file_spec()) {
                                    Some(local_path) => Some(local_path.display().to_string()),
                                    _ => None,
                                },
                                ..Default::default()
                            });
                        }
                    }
                    last_line_entry = line_entry;

                    if resolve_symbols {
                        if let Some(symbol) = address.symbol() {
                            dis_instr.symbol = Some(symbol.name().into());
                        }
                    }
                    dis_instr
                }
                Err(byte) => {
                    last_line_entry = None;
                    DisassembledInstruction {
                        address: format!("0x{:X}", arange.start),
                        instruction_bytes: Some(format!("{:02X}", byte)),
                        instruction: format!(".byte  {:02X}", byte),
                        presentation_hint: Some("invalid".into()),
                        ..Default::default()
                    }
                }
            };
            dis_instructions.push(dis_instsr);
        }
        let instr_count = instructions.len() as i64;
        if end_idx > instr_count {
            for i in 0..end_idx - instr_count {
                let addr = end_addr.wrapping_add(i as u64);
                dis_instructions.push(invalid_memory(addr));
            }
        }

        Ok(DisassembleResponseBody {
            instructions: dis_instructions,
        })
    }

    fn max_instruction_bytes(&self) -> u64 {
        let arch = self.target.triple().split("-").next().unwrap();
        if arch.starts_with("arm") || arch.starts_with("aarch64") || arch.starts_with("riscv") {
            return 4;
        }
        return 16;
    }

    // Read a memory range while handling unreadable regions.
    // If unreadable regions are encountered, the returned range will be a subset of the requested one.
    fn read_memory(&self, start_address: Address, count: u64) -> (Address, Vec<u8>) {
        let process = self.target.process();
        let mut buffer = Vec::new();
        buffer.resize(count as usize, 0);
        if let Ok(read) = process.read_memory(start_address, &mut buffer) {
            buffer.truncate(read);
            return (start_address, buffer);
        }
        // Binary search for the boundary between unreadable and readable areas
        let first_readable = partition_point(start_address..start_address + count, |addr| {
            let mut b = [0u8; 1];
            process.read_memory(addr, &mut b).is_err()
        });
        buffer.resize((start_address + count as u64 - first_readable) as usize, 0);
        match process.read_memory(first_readable, &mut buffer) {
            Ok(read) => {
                buffer.truncate(read);
                (first_readable, buffer)
            }
            Err(err) => {
                error!("Could not read memory: {err}");
                buffer.clear();
                (first_readable, buffer)
            }
        }
    }

    fn disassemble_bytes(&self, base_addr: Address, buffer: &[u8]) -> Vec<(Range<Address>, Result<SBInstruction, u8>)> {
        let flavor = self.target.disassembly_flavor();
        let mut base_addr = base_addr;
        let mut buffer = buffer;
        let mut dis_instructions = Vec::new();
        while buffer.len() > 0 {
            let sbaddr = SBAddress::from_load_address(base_addr, &self.target);
            let instructions = self.target.get_instructions(&sbaddr, buffer, flavor.as_deref());
            if instructions.len() > 0 {
                for instr in instructions.iter() {
                    let byte_size = instr.byte_size();
                    dis_instructions.push((base_addr..base_addr + byte_size as u64, Ok(instr)));
                    base_addr += byte_size as u64;
                    buffer = &buffer[byte_size..];
                }
            } else {
                // Cannot decode - skip one byte and try again
                dis_instructions.push((base_addr..base_addr + 1, Err(buffer[0])));
                buffer = &buffer[1..];
                base_addr += 1;
            }
        }
        dis_instructions
    }
}

fn invalid_memory(addr: Address) -> DisassembledInstruction {
    DisassembledInstruction {
        address: format!("0x{:X}", addr),
        instruction_bytes: Some("??".into()),
        presentation_hint: Some("invalid".into()),
        ..Default::default()
    }
}

/// Return the first value in the range for which the predicate retruns false.
fn partition_point<P>(range: Range<Address>, pred: P) -> Address
where
    P: Fn(Address) -> bool,
{
    let mut low = range.start;
    let mut high = range.end;
    while low < high {
        let mid = low + (high - low) / 2;
        if pred(mid) {
            low = mid + 1;
        } else {
            high = mid;
        }
    }
    low
}

#[test]
fn test_partition_point() {
    let p = partition_point(10..100, |i| i < 42);
    assert_eq!(p, 42);
}
