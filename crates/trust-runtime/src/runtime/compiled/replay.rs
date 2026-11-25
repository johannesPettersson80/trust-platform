//! Replaying a cycle trace in the simulator (specification 48 RU-60; specification 11 §6.8.1,
//! "Replaying a trace"): the build takes the trace's start state, carried as an online change
//! carries a running program's when the build's memory is laid out otherwise, and its forces, and
//! runs each recorded cycle at its logical time after the cycle's changes from outside, with its
//! task activations, its input image and its wall-clock readings; each cycle's outputs, or its
//! fault, are compared with the trace's.

use trust_plc_abi::FaultCode;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::native::change::carry;
use trust_runtime_core::native::image::{Image, TaskRecord};
use trust_runtime_core::native::variables::CompiledVariables;
use trust_runtime_core::trace::replay::{
    as_requested, differing_outputs, located_address, located_bytes, located_value, names_variable,
    path_value, recorded_image, same_layout, task_names, RecordedMemory,
};
use trust_runtime_core::trace::{
    decode, readings, Change, ChangeKind, Cycle, CycleFault, IoArea, Located,
};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_services::method_call::{typed_call, ALLOWANCE};
use trust_runtime_services::simulator_replay::{
    carried_note, Difference, FirstDifference, OutputDifference, ReplayKind, ReplayRefusal,
    ReplayReport,
};

use super::trace::cycle_fault;
use super::CompiledRuntime;
use crate::error::RuntimeError;

/// The recorded cycle that a replay's scan runs: its task activations, each a build task's index
/// and logical time, and its input image.
#[derive(Clone, Copy)]
pub(super) struct ReplayedCycle<'r> {
    pub(super) activations: &'r [(usize, Duration)],
    pub(super) inputs: &'r [u8],
}

impl CompiledRuntime {
    /// Replays the cycle trace `bytes` before the simulator's first cycle (specification 48 RU-60):
    /// the build takes the recorded memory, byte for byte when it lays its memory out as the
    /// recorded program did and otherwise as an online change carries a running program's values,
    /// and the recorded forces; then it applies each recorded cycle's changes from outside and runs
    /// the cycle at its logical time, the build's tasks of the recorded names at the recorded
    /// times, on the recorded input image with the recorded wall-clock readings, and compares its
    /// outputs, or its fault, with the trace's. A build that faults runs no further cycle.
    ///
    /// # Errors
    ///
    /// [`ReplayRefusal::SimulatorStarted`] after a cycle, and [`ReplayRefusal::TraceInvalid`] for a
    /// trace that does not read or whose start state does not carry into the build.
    pub(crate) fn replay_trace(&mut self, bytes: &[u8]) -> Result<ReplayReport, ReplayRefusal> {
        if self.cycle_counter > 0 {
            return Err(ReplayRefusal::SimulatorStarted);
        }
        let invalid = |reason: String| ReplayRefusal::TraceInvalid(reason);
        let decoded = decode(bytes).map_err(|error| invalid(error.to_string()))?;
        if let Some(cycle) = decoded.damaged_cycle {
            return Err(invalid(format!("its cycle {cycle} is damaged")));
        }
        let trace = decoded.trace;
        let recorded = task_names(&trace.image).map_err(|error| invalid(error.to_string()))?;
        // The recorded program's image reads the trace's memory and values (RU-60).
        let traced_bytes = recorded_image(&trace).map_err(|error| invalid(error.to_string()))?;
        let traced = Image::parse(&traced_bytes)
            .map_err(|_| invalid("its tables do not lay out as an image".to_string()))?;
        let image = self.image.image();
        let same = same_layout(&trace.image, &image);
        let header = image.header();
        let kind = ReplayKind::of(
            &trace.meta.sources_hash,
            trace.meta.target,
            &header.source_hash,
            header.target,
        );
        let build: Vec<String> = image
            .table::<TaskRecord>()
            .map(|tasks| {
                tasks
                    .iter()
                    .map(|task| image.string(task.name).unwrap_or_default().to_string())
                    .collect()
            })
            .unwrap_or_default();
        let tasks: Vec<Option<usize>> = recorded
            .iter()
            .map(|name| build.iter().position(|own| own.eq_ignore_ascii_case(name)))
            .collect();
        let mut notes: Vec<String> = recorded
            .iter()
            .zip(&tasks)
            .filter(|(_, task)| task.is_none())
            .map(|(name, _)| format!("the build has no task {name}: its activations are skipped"))
            .collect();
        if same {
            for (id, area) in &trace.start.areas {
                let written =
                    u8::try_from(*id).is_ok_and(|id| self.cycle.program_mut().write(id, 0, area));
                if !written {
                    return Err(invalid(format!("its start state does not fit area {id}")));
                }
            }
        } else {
            // A build laid out otherwise takes the recorded values as an online change carries a
            // running program's into a new one (RU-60, "The start"; specification 44 §12.2).
            let memory = RecordedMemory::new(traced, &trace.start.areas);
            let events = carry(&memory, self.cycle.program_mut()).map_err(|error| {
                invalid(format!(
                    "its start state does not carry into the build: {error}"
                ))
            })?;
            notes.extend(events.iter().map(carried_note));
        }
        // The trace's forces start in effect (RU-60, "The start").
        for force in &trace.start.forces {
            self.apply_change(force, &traced, &mut notes);
        }
        // The build reads the recorded wall clock while the replay runs, and its own again after.
        let _playback = Playback;
        let mut first_difference = None;
        let mut later_differing_cycles = 0;
        let mut cycles = 0;
        for (index, cycle) in trace.cycles.iter().enumerate() {
            // The changes come between the scans, as the runtime applied them: at the previous
            // cycle's logical time, and before the cycle's wall-clock readings, which belong to
            // its scan; a method call reads its own (RU-60).
            for change in &cycle.changes {
                self.apply_change(change, &traced, &mut notes);
            }
            self.set_current_time(Duration::from_nanos(cycle.time_ns));
            readings::play(&cycle.wall_clock_ms);
            let activations: Vec<(usize, Duration)> = cycle
                .activations
                .iter()
                .filter_map(|activation| {
                    let task = tasks.get(usize::try_from(activation.task).ok()?)?;
                    task.map(|task| (task, Duration::from_nanos(activation.now_ns)))
                })
                .collect();
            let result = self.execute_cycle_with(Some(ReplayedCycle {
                activations: &activations,
                inputs: &cycle.inputs,
            }));
            cycles += 1;
            if let Some(difference) = self.difference(&trace.located, &traced, cycle, &result) {
                if first_difference.is_none() {
                    first_difference = Some(FirstDifference {
                        cycle: u64::try_from(index).unwrap_or(u64::MAX),
                        time_ns: cycle.time_ns,
                        difference,
                    });
                } else {
                    later_differing_cycles += 1;
                }
            }
            if result.is_err() {
                // A build that faulted runs no further cycle, so each later recorded one differs.
                let after = trace.cycles.len() - index - 1;
                later_differing_cycles += u64::try_from(after).unwrap_or(u64::MAX);
                break;
            }
        }
        Ok(ReplayReport {
            cycles,
            kind,
            first_difference,
            later_differing_cycles,
            notes,
        })
    }

    /// Applies a change from outside as the runtime applied it before the recorded cycle (RU-60,
    /// "Each cycle"): a variable's write through the checked write; a force set or released through
    /// Live Values, which holds it from the cycle's start on; an address's write into the process
    /// image; a method call between the scans. A recorded value is read by the recorded program's
    /// image `traced`, and a number is given back as the literal the request gave
    /// ([`as_requested`]), which the checked write types for the build's variable. A trace names a
    /// forced variable by its path, a global's name or an instance member's path as the debugger
    /// names it, and Live Values forces a variable named so through the checked write, which
    /// reaches either; the checked write takes the forced value first, so that a value the build
    /// refuses is noted rather than dropped at the cycle's start. A variable, address or method
    /// the build lacks, or a value it refuses, is noted once and skipped.
    fn apply_change(&mut self, change: &Change, traced: &Image<'_>, notes: &mut Vec<String>) {
        let image = self.image.image();
        let lacks = |what: &str, path: &str| {
            format!("the build has no {what} {path}: its changes are skipped")
        };
        match &change.kind {
            ChangeKind::VariableWrite { path, value, .. } => {
                let Some(value) = self.requested(traced, path, value, notes) else {
                    return;
                };
                if CompiledVariables::write_named(self.cycle.program_mut(), path, &value).is_err() {
                    note_once(notes, &format!("the build refuses a write of {path}"));
                }
            }
            ChangeKind::VariableForce { path, value, .. } => {
                let Some(value) = self.requested(traced, path, value, notes) else {
                    return;
                };
                if CompiledVariables::write_named(self.cycle.program_mut(), path, &value).is_err() {
                    return note_once(notes, &format!("the build refuses a force of {path}"));
                }
                let forced = self
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.try_force_global(path.as_str(), value).is_ok());
                if !forced {
                    note_once(notes, &format!("the build cannot force {path}"));
                }
            }
            ChangeKind::VariableRelease { path } => {
                if let Some(debug) = &self.debug {
                    debug.release_global(path);
                }
            }
            ChangeKind::AddressWrite {
                area,
                bit,
                offset,
                bytes,
            } => {
                let (text, address) = address_change(*area, *bit, *offset, bytes);
                let Some((address, value)) = address else {
                    return note_once(notes, &lacks("address", &text));
                };
                if CompiledVariables::write_image(self.cycle.program_mut(), &address, value)
                    .is_err()
                {
                    note_once(notes, &format!("the build refuses a write of {text}"));
                }
            }
            ChangeKind::AddressForce {
                area,
                bit,
                offset,
                bytes,
            } => {
                let (text, address) = address_change(*area, *bit, *offset, bytes);
                let Some((address, value)) = address else {
                    return note_once(notes, &lacks("address", &text));
                };
                let forced = self
                    .debug
                    .as_ref()
                    .is_some_and(|debug| debug.try_force_io(address, value).is_ok());
                if !forced {
                    note_once(notes, &format!("the build cannot force {text}"));
                }
            }
            ChangeKind::AddressRelease {
                area,
                bit,
                offset,
                size,
            } => {
                let bytes = vec![0; usize::try_from(*size).unwrap_or(1).max(1)];
                if let (Some(debug), (_, Some((address, _)))) =
                    (&self.debug, address_change(*area, *bit, *offset, &bytes))
                {
                    debug.release_io(&address);
                }
            }
            ChangeKind::MethodCall {
                instance,
                interface,
                method,
                inputs,
                readings: call_readings,
            } => {
                let inputs: Vec<(&str, &str)> = inputs
                    .iter()
                    .map(|(name, literal)| (name.as_str(), literal.as_str()))
                    .collect();
                // The call reads the wall clock as it read it when it was recorded; one that read
                // none reads the last reading the replay gave (RU-60).
                if !call_readings.is_empty() {
                    readings::play(call_readings);
                }
                let called = typed_call(&image, instance, interface, method, &inputs)
                    .map_err(|_| ())
                    .and_then(|call| {
                        self.call_method(
                            &call.instance,
                            &call.interface,
                            &call.method,
                            &call.inputs,
                            ALLOWANCE,
                        )
                        .map_err(|_| ())
                    });
                if called.is_err() {
                    note_once(
                        notes,
                        &format!("the build cannot call {method} of {instance}"),
                    );
                }
            }
        }
    }

    /// The value of a recorded write or force of the variable `path`, `bytes` read by the recorded
    /// program's image `traced`, as the request gave it ([`as_requested`]); none, and a note,
    /// when the build lacks the variable or the trace's bytes do not read.
    fn requested(
        &self,
        traced: &Image<'_>,
        path: &str,
        bytes: &[u8],
        notes: &mut Vec<String>,
    ) -> Option<Value> {
        if !names_variable(&self.image.image(), path) {
            note_once(
                notes,
                &format!("the build has no variable {path}: its changes are skipped"),
            );
            return None;
        }
        let Some(value) = path_value(traced, path, bytes) else {
            note_once(
                notes,
                &format!("the trace's value of {path} does not read: its changes are skipped"),
            );
            return None;
        };
        Some(as_requested(value))
    }

    /// How the cycle just replayed differs from the recorded `cycle`: its outputs, by the trace's
    /// located output variables, each side's bytes read as the variable's type in the recorded
    /// program's image `traced`, or its fault, by the fault's code; none when it matches.
    fn difference(
        &self,
        located: &[Located],
        traced: &Image<'_>,
        cycle: &Cycle,
        result: &Result<(), RuntimeError>,
    ) -> Option<Difference> {
        let image = self.image.image();
        match (&cycle.fault, result) {
            (None, Ok(())) => {
                let build = &self.cycle.program().outputs;
                let value = |variable: &Located, outputs: &[u8]| {
                    located_bytes(variable, outputs)
                        .and_then(|bytes| located_value(traced, variable, &bytes))
                };
                let outputs: Vec<OutputDifference> =
                    differing_outputs(located, &cycle.outputs, build)
                        .into_iter()
                        .map(|variable| OutputDifference {
                            path: variable.path.clone(),
                            address: located_address(variable),
                            trace: value(variable, &cycle.outputs),
                            build: value(variable, build),
                        })
                        .collect();
                (!outputs.is_empty()).then_some(Difference::Outputs(outputs))
            }
            (Some(recorded), Err(_)) => {
                let fault = cycle_fault(&image, &self.cycle.program().last_fault());
                (fault.code != recorded.code).then(|| Difference::Faults {
                    trace: Some(fault_text(recorded)),
                    build: Some(fault_text(&fault)),
                })
            }
            (Some(recorded), Ok(())) => Some(Difference::Faults {
                trace: Some(fault_text(recorded)),
                build: None,
            }),
            (None, Err(error)) => Some(Difference::Faults {
                trace: None,
                build: Some(error.to_string()),
            }),
        }
    }
}

/// Adds `note` to `notes`, once.
fn note_once(notes: &mut Vec<String>, note: &str) {
    if !notes.iter().any(|noted| noted == note) {
        notes.push(note.to_string());
    }
}

/// The process image's address that a trace's address change names, as IEC 61131-3 writes it, and
/// the address with the value of its bytes as a write of it takes them: a bit as BOOL, one, two,
/// four and eight bytes as BYTE, WORD, DWORD and LWORD (specification 44 §7.3); none for another
/// length.
fn address_change(
    area: IoArea,
    bit: Option<u8>,
    offset: u32,
    bytes: &[u8],
) -> (String, Option<(IoAddress, Value)>) {
    let area = match area {
        IoArea::Input => 'I',
        IoArea::Output => 'Q',
        IoArea::Memory => 'M',
    };
    let typed = match (bit, bytes) {
        (Some(bit), [byte]) => Some((format!("%{area}X{offset}.{bit}"), Value::Bool(*byte == 1))),
        (None, [byte]) => Some((format!("%{area}B{offset}"), Value::Byte(*byte))),
        (None, &[a, b]) => Some((
            format!("%{area}W{offset}"),
            Value::Word(u16::from_le_bytes([a, b])),
        )),
        (None, &[a, b, c, d]) => Some((
            format!("%{area}D{offset}"),
            Value::DWord(u32::from_le_bytes([a, b, c, d])),
        )),
        (None, &[a, b, c, d, e, f, g, h]) => Some((
            format!("%{area}L{offset}"),
            Value::LWord(u64::from_le_bytes([a, b, c, d, e, f, g, h])),
        )),
        _ => None,
    };
    match typed {
        Some((text, value)) => {
            let address = IoAddress::parse(&text).ok().map(|address| (address, value));
            (text, address)
        }
        None => (format!("%{area}{offset} ({} bytes)", bytes.len()), None),
    }
}

/// Plays the trace's wall-clock readings to the build while it lives, and ends them when the
/// replay ends however it ends.
struct Playback;

impl Drop for Playback {
    fn drop(&mut self) {
        readings::stop_playing();
    }
}

/// A recorded fault as the report names it: its code, the POU and the statement that raised it.
fn fault_text(fault: &CycleFault) -> String {
    let code = FaultCode::from_raw(fault.code).map_or_else(
        || format!("fault code {}", fault.code),
        |code| format!("{code:?}"),
    );
    format!(
        "{code} in POU {} at statement {}",
        fault.pou, fault.statement
    )
}
