use crate::prelude::*;

use super::variables::Container;

use std::cmp;
use std::ops::Range;

use adapter_protocol::*;
use lldb::*;
use regex_lite::Regex;

#[derive(Debug, Clone)]
pub(super) struct StepInTargetInternal {
    target_fn: SBFunction,
    /// Range of addresses occupied by the statement for which this target was computed
    stmt_range: Range<Address>,
}

impl super::DebugSession {
    pub(super) fn handle_step_in_targets(
        &mut self,
        args: StepInTargetsArguments,
    ) -> Result<StepInTargetsResponseBody, Error> {
        let Some(ref frame) = self.var_refs.get(args.frame_id) else {
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

        // The typical case we aim to handle:
        // ```
        //    AAA(BBB(),  <-- execution stopped here; the user wants to step into AAA, bypassing BBB and CCC
        //        CCC())
        // ```
        // In this scenario, the current PC will typically point to the `call BBB` instruction,
        // with `call CCC` and `call AAA` following.  Since `call CCC` is on a different line, `call AAA` will
        // likely be in a separate code range from `call BBB`, with a separate LineEntry. As a result, scanning
        // only the address range of the current LineEntry (that of `call BBB`) won't discover `call AAA`.
        //
        // To handle this, we determine the address range covering the entire statement by looking for all LineEntries
        // with the same line number as the current one, then find the largest end address.  The address range of the
        // statement is then PC..max_end_addr.  Of course, this assumes that instructions are ordered in the logical
        // order of function calls, but, in debug builds, this usually holds true.
        let mut max_end_addr = 0;
        for le in cu.line_entries() {
            if le.line() == curr_le.line() && le.file_spec().filename() == curr_le.file_spec().filename() {
                let start_address = le.start_address().load_address(&self.target);
                let end_address = le.end_address().load_address(&self.target);
                debug!("{le:?}, range: {start_address:X}..{end_address:X}");
                max_end_addr = cmp::max(max_end_addr, end_address);
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
            match self.control_flow_kind(&instr) {
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

    fn control_flow_kind(&self, instr: &SBInstruction) -> InstructionControlFlowKind {
        let kind = instr.control_flow_kind(&self.target);
        if kind == InstructionControlFlowKind::Unknown {
            if self.target.triple().starts_with("arm") {
                match instr.mnemonic(&self.target) {
                    "bl" => InstructionControlFlowKind::Call,
                    "b" => InstructionControlFlowKind::Jump,
                    _ => InstructionControlFlowKind::Other,
                }
            } else {
                kind
            }
        } else {
            kind
        }
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
                self.control_flow_kind(&target_instr)
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
                log_errors!(self.step_into_target(&thread, &step_target));
                self.notify_process_stopped();
            } else {
                thread.step_into(RunMode::OnlyDuringStepping)?;
            }
        }
        Ok(())
    }

    fn step_into_target(&self, thread: &SBThread, step_target: &StepInTargetInternal) -> Result<(), Error> {
        let _token = lldb_stub::v16.resolve()?;
        self.with_sync_mode(|| {
            let start_frame = thread.frame_at_index(0);
            thread.step_into(RunMode::OnlyThisThread)?;
            loop {
                if self.current_cancellation.is_cancelled() {
                    bail!("Cancelled");
                }

                let curr_frame = thread.frame_at_index(0);
                if !curr_frame.is_valid() {
                    bail!("Invalid current frame");
                }

                if curr_frame.function() == step_target.target_fn {
                    return Ok(()); // Success!
                }

                let instr = self.target.read_instructions(&curr_frame.pc_address(), 1, None).instruction_at_index(0);
                if let InstructionControlFlowKind::Jump | InstructionControlFlowKind::FarJump =
                    self.control_flow_kind(&instr)
                {
                    // We are on a trampoline that jumps to the actual function
                    thread.step_into(RunMode::OnlyThisThread)?;
                    continue;
                }

                // Figure out where we landed
                let start_frame_idx =
                    thread.frames().take(10).enumerate().find_map(
                        |(idx, frame)| {
                            if frame == start_frame {
                                Some(idx)
                            } else {
                                None
                            }
                        },
                    );
                match start_frame_idx {
                    None => {
                        // We are in a parent frame of start_frame - stop
                        bail!("Stepped out of start_frame");
                    }
                    Some(0) => {
                        // Still in the start_frame
                        if !step_target.stmt_range.contains(&curr_frame.pc()) {
                            bail!("Stepped out of stmt_range");
                        }
                        thread.step_into(RunMode::OnlyThisThread)?; // Try again
                    }
                    Some(_) => {
                        // We stepped in, but not into the function we need - step back out
                        thread.step_out()?;
                    }
                }
            }
        })
    }
}
