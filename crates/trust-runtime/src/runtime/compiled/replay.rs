//! Replaying a cycle trace in the simulator (specification 48 RU-60; specification 11 §6.8.1,
//! "Replaying a trace"): the build takes the trace's start state and runs each recorded cycle at
//! its logical time, with its task activations and its input image; each cycle's outputs, or its
//! fault, are compared with the trace's.

use trust_plc_abi::FaultCode;
use trust_runtime_core::native::image::TaskRecord;
use trust_runtime_core::trace::replay::{
    differing_outputs, located_address, located_bytes, located_value, same_layout, task_names,
};
use trust_runtime_core::trace::{decode, Cycle, CycleFault, Located};
use trust_runtime_core::value::Duration;
use trust_runtime_services::simulator_replay::{
    Difference, FirstDifference, OutputDifference, ReplayKind, ReplayRefusal, ReplayReport,
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
    /// the build takes the recorded memory byte for byte, then runs each recorded cycle at its
    /// logical time, the build's tasks of the recorded names at the recorded times, on the recorded
    /// input image, and compares its outputs, or its fault, with the trace's. A build that faults
    /// runs no further cycle.
    ///
    /// # Errors
    ///
    /// [`ReplayRefusal::SimulatorStarted`] after a cycle, [`ReplayRefusal::TraceInvalid`] for a
    /// trace that does not read, and [`ReplayRefusal::Unavailable`] for a build whose memory is laid
    /// out unlike the recorded program's.
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
        let image = self.image.image();
        if !same_layout(&trace.image, &image) {
            return Err(ReplayRefusal::Unavailable(
                "the build's area, symbol or type tables differ from the trace's: a replay of a \
                 changed build is not available yet"
                    .to_string(),
            ));
        }
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
        for (id, area) in &trace.start.areas {
            let written =
                u8::try_from(*id).is_ok_and(|id| self.cycle.program_mut().write(id, 0, area));
            if !written {
                return Err(invalid(format!("its start state does not fit area {id}")));
            }
        }
        if !trace.start.forces.is_empty() {
            notes.push("the forces in effect at the trace's start are not applied".to_string());
        }
        let mut first_difference = None;
        let mut later_differing_cycles = 0;
        let mut cycles = 0;
        for (index, cycle) in trace.cycles.iter().enumerate() {
            note_once(
                &mut notes,
                !cycle.changes.is_empty(),
                "the trace's changes from outside are not applied",
            );
            note_once(
                &mut notes,
                !cycle.wall_clock_ms.is_empty(),
                "the trace's readings of the wall clock are not given to the build",
            );
            let activations: Vec<(usize, Duration)> = cycle
                .activations
                .iter()
                .filter_map(|activation| {
                    let task = tasks.get(usize::try_from(activation.task).ok()?)?;
                    task.map(|task| (task, Duration::from_nanos(activation.now_ns)))
                })
                .collect();
            self.set_current_time(Duration::from_nanos(cycle.time_ns));
            let result = self.execute_cycle_with(Some(ReplayedCycle {
                activations: &activations,
                inputs: &cycle.inputs,
            }));
            cycles += 1;
            if let Some(difference) = self.difference(&trace.located, cycle, &result) {
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

    /// How the cycle just replayed differs from the recorded `cycle`: its outputs, by the trace's
    /// located output variables, or its fault, by the fault's code; none when it matches.
    fn difference(
        &self,
        located: &[Located],
        cycle: &Cycle,
        result: &Result<(), RuntimeError>,
    ) -> Option<Difference> {
        let image = self.image.image();
        match (&cycle.fault, result) {
            (None, Ok(())) => {
                let build = &self.cycle.program().outputs;
                let value = |variable: &Located, outputs: &[u8]| {
                    located_bytes(variable, outputs)
                        .and_then(|bytes| located_value(&image, variable, &bytes))
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

/// Adds `note` to `notes` when `holds`, once.
fn note_once(notes: &mut Vec<String>, holds: bool, note: &str) {
    if holds && !notes.iter().any(|noted| noted == note) {
        notes.push(note.to_string());
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
