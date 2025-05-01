use crate::prelude::*;

use super::variables::Container;
use crate::handles::{self};

use std::cmp;
use std::ops::Range;

use adapter_protocol::*;
use lldb::*;
use regex_lite::Regex;

#[derive(Debug, Clone)]
pub(super) struct StepInTargetInternal {
    target_fn: SBFunction,
    stmt_range: Range<Address>,
}

impl super::DebugSession {
    pub(super) fn handle_step_in_targets(
        &mut self,
        args: StepInTargetsArguments,
    ) -> Result<StepInTargetsResponseBody, Error> {
        let handle = handles::from_i64(args.frame_id)?;
        let Some(ref frame) = self.var_refs.get(handle) else {
            bail!("Invalid frame id.");
        };
        let Container::StackFrame(frame) = frame else {
            bail!("Invalid frame id.");
        };
        let Some(curr_le) = frame.line_entry() else {
            bail!("No line entry for frame.");
        };
        let Some(cu) = frame.compile_uint() else {
            bail!("No compile unit for frame.");
        };
        let _token = lldb_stub::v16.resolve()?;

        // This is the typical case we aim to handle:
        // ```
        //    AAA(BBB(),  <-- execution stopped on this line; the user wants to step into AAA, bypassing BBB and CCC
        //        CCC())
        // ```
        // In this scenario, the current PC will typically point to the `call BBB` instruction,
        // with `call CCC` and `call AAA` following. Since `call CCC` is on a different line, `call AAA` will
        // likely be in a separate code range from `call BBB`, with a separate LineEntry. As a result, scanning
        // only the address range of the current LineEntry (that of `call BBB`) won't discover `call AAA`.
        // To handle this, we determine the address range covering the entire statement by looking for all LineEntries
        // with the same line number as the current one, then find the largest end address.
        // This, of course, assumes the instruction ordering is as described above, but in debug builds, this is usually the case.
        let mut max_end_addr = 0;
        for le in cu.line_entries() {
            if le.line() == curr_le.line() && le.file_spec().filename() == curr_le.file_spec().filename() {
                debug!(
                    "{:?}, range: {:X}..{:X}",
                    le,
                    le.start_address().load_address(&self.target),
                    le.end_address().load_address(&self.target)
                );
                let end_eddr = le.end_address().load_address(&self.target);
                max_end_addr = cmp::max(max_end_addr, end_eddr);
            }
        }
        // Get instructions for address range PC..max_end_addr
        let instr_count = (max_end_addr - frame.pc()) as u32;
        let instructions = self.target.read_instructions(&frame.pc_address(), instr_count, Some("att"));
        let instructions = instructions
            .iter()
            .take_while(|instr| instr.address().load_address(&self.target) < max_end_addr);
        // Find `call` instructions and try to determine their targets.
        let mut target_fns = Vec::new();
        for instr in instructions {
            match instr.control_flow_kind(&self.target) {
                InstructionControlFlowKind::Call | InstructionControlFlowKind::FarCall => {
                    if let Ok(func) = self.call_target_function(&instr) {
                        // Include only targets that have line numbers debug info
                        if func.is_valid() && func.start_address().line_entry().is_some() {
                            // Deduplicate since we don't currently distinguish between invocation instances.
                            if !target_fns.contains(&func) {
                                target_fns.push(func);
                            }
                        }
                    }
                }
                _ => (),
            }
        }
        let curr_pc = frame.pc();
        let mut targets = Vec::new();
        for target_fn in target_fns {
            let target_id = self.step_in_targets.len() as i64;
            let display_name = target_fn.display_name().to_owned();
            self.step_in_targets.push(StepInTargetInternal {
                target_fn: target_fn,
                stmt_range: curr_pc..max_end_addr,
            });
            targets.push(StepInTarget {
                id: target_id,
                label: display_name,
                ..Default::default()
            });
        }

        Ok(StepInTargetsResponseBody { targets })
    }

    // Try to determine target function of a call instruction.
    fn call_target_function(&self, instruction: &SBInstruction) -> Result<SBFunction, Error> {
        let address = self.instruction_target_address(instruction)?;
        if let Some(func) = address.function() {
            return Ok(func);
        }
        // Check if there's a `jmp <immed>` at that address.
        let target_instr = self.target.read_instructions(&address, 1, None).instruction_at_index(0);
        if target_instr.is_valid() {
            if let InstructionControlFlowKind::Jump | InstructionControlFlowKind::FarJump =
                target_instr.control_flow_kind(&self.target)
            {
                let address = self.instruction_target_address(&target_instr)?;
                if let Some(func) = address.function() {
                    return Ok(func);
                }
            }
        }
        bail!("Could not determine call target")
    }

    // Try to determine the target address of a call or a jmp instruction.
    fn instruction_target_address(&self, instruction: &SBInstruction) -> Result<SBAddress, Error> {
        let operands = instruction.operands(&self.target);
        lazy_static::lazy_static! {
            // 0x12345
            static ref IMMED: Regex = Regex::new(r"^0[xX][[:xdigit:]]+$").unwrap();
            // *0x1234(%rip)
            static ref RIP_REL: Regex = Regex::new(r"^\*(0[xX][[:xdigit:]]+)\(%rip\)$").unwrap();
        }
        let load_addr = if IMMED.is_match(operands) {
            parse_int::parse::<u64>(operands)?
        } else if let Some(caps) = RIP_REL.captures(operands) {
            let offset = parse_int::parse::<u64>(&caps[1])?;
            let mut got_ptr = instruction.address();
            got_ptr.add_offset(offset.wrapping_add(instruction.byte_size() as u64));
            let mut buffer = [0u8; 8];
            self.target.read_memory(&got_ptr, &mut buffer)?;
            u64::from_le_bytes(buffer)
        } else {
            bail!("Unrecognized operand form");
        };
        Ok(SBAddress::from_load_address(load_addr, &self.target))
    }

    pub(super) fn handle_step_in(&mut self, args: StepInArguments) -> Result<(), Error> {
        let thread = match self.target.process().thread_by_id(args.thread_id as ThreadID) {
            Some(thread) => thread,
            None => {
                error!("Received invalid thread id in step-in request.");
                bail!("Invalid thread id.")
            }
        };
        let frame = thread.frame_at_index(0);

        let step_target = if let Some(id) = args.target_id {
            self.step_in_targets.get(id as usize).cloned() //.
        } else {
            None
        };

        self.before_resume();

        let step_instruction = match args.granularity {
            Some(SteppingGranularity::Instruction) => true,
            Some(SteppingGranularity::Line) | Some(SteppingGranularity::Statement) => false,
            None => self.in_disassembly(&frame),
        };

        if step_instruction {
            thread.step_instruction(false)?;
        } else {
            if let Some(step_target) = step_target {
                let target_name = step_target.target_fn.name();
                self.with_sync_mode(||
                    // SBThread::StepInto(target...) will step only within the range of the LineEntry covering the
                    // current PC (the variant with `end_line` works only if the compiler generated a line number index).
                    // Since the target function might belong to a different LineEntry, we attempt
                    // step-ins until execution moves outside the statement range computed above.
                    // This will happen in two cases: either we successfully step into the target function, or
                    // we step outside the call site statement.
                    loop {
                        let result = thread.step_into_target(target_name, None, RunMode::OnlyDuringStepping);
                        if result.is_err() || self.current_cancellation.is_cancelled() {
                            break;
                        }
                        let frame = thread.frame_at_index(0);
                        if !step_target.stmt_range.contains(&frame.pc()) {
                            break;
                        }
                    });
                self.notify_process_stopped();
            } else {
                thread.step_into(RunMode::OnlyDuringStepping)?;
            }
        }
        Ok(())
    }
}
