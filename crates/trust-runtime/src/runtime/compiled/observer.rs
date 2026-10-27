//! What a computer records of a compiled program's scan, at the phases the compiled cycle reports
//! (specification 44 §8.7, §8.8), as the bytecode engine's hosted cycle records them
//! (`installed_cycle.rs`): the cycle's and each task's events, the task, program and overrun
//! metrics, the debugger's thread, the Live Values, the MQTT tags and the ADS and OPC UA clients at
//! their access points (specification 44 §13.3) and the I/O snapshot after the output sample.

use std::sync::Mutex;
use std::time::Instant;

use smol_str::SmolStr;
use trust_runtime_core::cycle::{CycleHooks, CyclePhase};
use trust_runtime_core::io_image::{FlatImages, PreparedIoBinding};
use trust_runtime_core::memory::Generation;
use trust_runtime_core::native::variables::{CompiledAccessPoint, CompiledVariables};
use trust_runtime_services::live_values::io_snapshot::value_type_name;
use trust_runtime_services::runner::TaskDescriptor;

use super::super::ads_subsystem::AdsSubsystem;
use super::super::io_subsystem::IoDriverEntry;
use super::super::link_subsystem::{LinkSources, LinkSubsystem};
use super::super::metrics_subsystem::MetricsSubsystem;
use super::super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::process_io::lock_drivers;
use super::tags::{apply_input_tags, capture_output_tags, ExtraImages, TagLink};
use super::CompiledDebugger;
use crate::debug::{DebugControl, RuntimeEvent};
use crate::error::RuntimeError;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
use crate::runtime::program_access::Access;
use crate::value::Duration;

/// The computer's [`CycleHooks`] for one scan of a compiled program.
pub(super) struct HostedCompiledScan<'a> {
    pub(super) metrics: &'a MetricsSubsystem,
    pub(super) debug: Option<DebugControl>,
    /// The program's debugger, which takes the thread of each task that starts.
    pub(super) debugger: Option<&'a CompiledDebugger>,
    /// The ADS and OPC UA clients, whose samples reach the program after its inputs and whose
    /// outputs leave it after its scan.
    pub(super) ads: &'a mut AdsSubsystem,
    pub(super) opcua: &'a mut OpcUaClientSubsystem,
    /// The MQTT tags and the extra image bytes they lie in.
    pub(super) tags: &'a [TagLink],
    pub(super) extra: &'a Mutex<ExtraImages>,
    pub(super) tasks: &'a [TaskDescriptor],
    /// The program instance of each entry of the image, by which a program call is recorded.
    pub(super) programs: &'a [SmolStr],
    pub(super) cycle: u64,
    pub(super) generation: Generation,
    pub(super) task_timer: Option<Instant>,
    pub(super) program_timer: Option<Instant>,
    /// The deadline of the scan's output commit, which the watchdog arms; past it the drivers are
    /// not written (specification 10 §4.7, step 6). A debugger's hold during the scan moves it on
    /// by the hold's time, which no watchdog counts.
    pub(super) output_deadline: Option<Instant>,
    /// The debugger's total hold time when the scan started.
    pub(super) held_at_start: std::time::Duration,
    /// The tasks that ran in this scan, by name, each with the activations it missed before it
    /// (Control Studio's recordings and runtime facts, specification 35 A2.4, A4.1).
    pub(super) task_runs: Vec<(SmolStr, u64)>,
    /// The activations each task missed before its activation in this scan, by the task's index.
    pub(super) missed: Vec<u64>,
    /// Whether the program drives physical I/O, so that a Control Studio commit needs the approval
    /// its request carries (specification 35 A2.4).
    pub(super) physical_target: bool,
    /// The program's `Comm.Link` blocks and the drivers whose links they show (specification 42).
    pub(super) links: &'a mut LinkSubsystem,
    pub(super) drivers: &'a Mutex<Vec<IoDriverEntry>>,
    /// The bytes of the input image, the MQTT tags' included, which a link's test reads into.
    pub(super) input_len: usize,
}

/// The name of the task at a task boundary, `$background` for the programs outside every task, as
/// Control Studio binds a loop to its task (control-studio-runtime-protocol E6.2).
fn boundary_task(tasks: &[TaskDescriptor], task: Option<usize>) -> Result<SmolStr, RuntimeError> {
    match task {
        Some(index) => Ok(self::task(tasks, index)?.name.clone()),
        None => Ok(SmolStr::new_static("$background")),
    }
}

/// Task `index` of `tasks`.
fn task(tasks: &[TaskDescriptor], index: usize) -> Result<&TaskDescriptor, RuntimeError> {
    tasks.get(index).ok_or(RuntimeError::InvalidExecutionState)
}

impl HostedCompiledScan<'_> {
    /// The debugger thread of task `index`, as the tasks are numbered in their order and a repeated
    /// name keeps its first task's (specification 13, "Multi-Task Thread Model").
    fn task_thread(&self, index: usize) -> Option<u32> {
        let name = &self.tasks.get(index)?.name;
        let mut thread = 0_u32;
        for (position, task) in self.tasks.iter().enumerate() {
            if self.tasks[..position]
                .iter()
                .any(|earlier| earlier.name == task.name)
            {
                continue;
            }
            thread = thread.checked_add(1)?;
            if task.name == *name {
                return Some(thread);
            }
        }
        None
    }

    /// The thread of the programs that run outside any task, after the last task's.
    fn background_thread(&self) -> Option<u32> {
        let mut threads = 0_u32;
        for (position, task) in self.tasks.iter().enumerate() {
            if !self.tasks[..position]
                .iter()
                .any(|earlier| earlier.name == task.name)
            {
                threads = threads.checked_add(1)?;
            }
        }
        threads.checked_add(1)
    }
}

/// The I/O snapshot of the scan `cycle` that ended: each binding's value read from the process
/// images the scan left, with the forced addresses.
pub(super) fn io_snapshot(
    bindings: &[PreparedIoBinding],
    binding_names: &[Option<SmolStr>],
    cycle: u64,
    images: &FlatImages,
    debug: &DebugControl,
) -> IoSnapshot {
    let mut snapshot = IoSnapshot {
        scan: Some(cycle),
        forced: debug
            .forced_snapshot()
            .io
            .into_iter()
            .map(|(address, _)| address)
            .collect(),
        ..IoSnapshot::default()
    };
    for (binding, name) in bindings.iter().zip(binding_names) {
        // The image holds the bits; the binding's codec gives them the variable's type, as the
        // snapshot of a chip decodes them.
        let value = match images
            .read(&binding.address)
            .and_then(|raw| binding.codec.decode(raw))
        {
            Ok(value) => IoSnapshotValue::Value(value),
            Err(error) => IoSnapshotValue::Error(error.to_string()),
        };
        let entry = IoSnapshotEntry {
            name: name.clone(),
            address: binding.address.clone(),
            value_type: None,
            value_type_name: binding
                .codec
                .wire_type
                .and_then(value_type_name)
                .map(SmolStr::new),
            value,
            source: None,
        };
        match binding.address.area {
            crate::memory::IoArea::Input => snapshot.inputs.push(entry),
            crate::memory::IoArea::Output => snapshot.outputs.push(entry),
            crate::memory::IoArea::Memory => snapshot.memory.push(entry),
        }
    }
    snapshot
}

impl CycleHooks for HostedCompiledScan<'_> {
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        _images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        match phase {
            CyclePhase::CycleStart => {
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::CycleStart {
                        cycle: self.cycle,
                        time: now,
                    });
                }
            }
            CyclePhase::TaskOverrun { index, missed } => {
                let task = task(self.tasks, index)?;
                if let Some(slot) = self.missed.get_mut(index) {
                    *slot = missed;
                }
                self.metrics.record_overrun(&task.name, missed);
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TaskOverrun {
                        name: task.name.clone(),
                        missed,
                        time: now,
                    });
                }
            }
            CyclePhase::TaskStart(index) => {
                let task = task(self.tasks, index)?;
                self.task_runs.push((
                    task.name.clone(),
                    self.missed.get(index).copied().unwrap_or(0),
                ));
                self.task_timer = self.metrics.start_timer();
                if let Some(debugger) = self.debugger {
                    debugger.set_thread(self.task_thread(index));
                }
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TaskStart {
                        name: task.name.clone(),
                        priority: task.priority,
                        time: now,
                    });
                }
            }
            CyclePhase::TaskEnd(index) => {
                let task = task(self.tasks, index)?;
                if let Some(start) = self.task_timer.take() {
                    self.metrics.record_task(&task.name, start.elapsed());
                }
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TaskEnd {
                        name: task.name.clone(),
                        priority: task.priority,
                        time: now,
                    });
                }
            }
            CyclePhase::BackgroundStart => {
                if let Some(debugger) = self.debugger {
                    debugger.set_thread(self.background_thread());
                }
            }
            CyclePhase::ProgramStart(_) => self.program_timer = self.metrics.start_timer(),
            // Each invoked program is a call of its instance, a faulted one too (specification 11,
            // runtime metrics).
            CyclePhase::ProgramEnd(entry) => {
                if let Some(start) = self.program_timer.take()
                    && let Some(name) = self.programs.get(entry).filter(|name| !name.is_empty())
                {
                    self.metrics.record_call("program", name, start.elapsed());
                }
            }
            CyclePhase::AfterOutputSample => {
                let held = self
                    .debug
                    .as_ref()
                    .map_or(
                        std::time::Duration::ZERO,
                        DebugControl::watchdog_pause_elapsed,
                    )
                    .saturating_sub(self.held_at_start);
                if self
                    .output_deadline
                    .map(|deadline| deadline.checked_add(held).unwrap_or(deadline))
                    .is_some_and(|deadline| Instant::now() >= deadline)
                {
                    return Err(RuntimeError::WatchdogTimeout);
                }
                // The scan's I/O snapshot reaches Live Values, the recordings and the experiments
                // once, at the scan's end (`CompiledRuntime::execute_cycle`).
            }
            CyclePhase::BeforeInputSample
            | CyclePhase::BackgroundEnd
            | CyclePhase::BeforeOutputSample
            | CyclePhase::CycleEnd => {}
        }
        Ok(())
    }

    /// The Live Values reach the program through its symbol table (specification 44 §13.3), and
    /// after them, as on the bytecode engine, the MQTT tags' and the clients' samples after the
    /// inputs and their outputs after the scan, the clients at the logical time in milliseconds.
    fn on_compiled_access(
        &mut self,
        point: CompiledAccessPoint,
        now: Duration,
        variables: &mut dyn CompiledVariables,
    ) -> Result<(), RuntimeError> {
        if let Some(debug) = &self.debug {
            // Control Studio's experiment writes of this scan, before the Live Values, which a
            // force then holds over them (specification 35 A3.5).
            if point == CompiledAccessPoint::CycleStart {
                debug.apply_control_writes(&mut Access(variables), self.generation, now);
            }
            debug.apply_compiled_live_values(point, self.generation, variables, now)?;
        }
        let now_ms = u64::try_from(now.as_millis()).unwrap_or(0);
        match point {
            // Control Studio commits a pending parameter set or controller transfer at the start of
            // the task that runs its controller, and confirms or reverts it once the task has run,
            // before any output is written (specification 35 A6.3, A6.8; specification 44 §13.3).
            CompiledAccessPoint::TaskStart(task) => {
                if let Some(debug) = &self.debug
                    && (debug.has_parameter_transaction() || debug.has_transfer_transaction())
                {
                    let name = boundary_task(self.tasks, task)?;
                    debug.commit_parameters(&name, &mut Access(variables), self.physical_target);
                    debug.commit_transfer(&name, &mut Access(variables), self.physical_target);
                }
                Ok(())
            }
            CompiledAccessPoint::TaskEnd(task) => {
                if let Some(debug) = &self.debug {
                    let name = boundary_task(self.tasks, task)?;
                    debug.confirm_parameters(&name, &mut Access(variables));
                    debug.confirm_transfer_task(&name, &mut Access(variables));
                }
                Ok(())
            }
            CompiledAccessPoint::AfterInputs => {
                apply_input_tags(self.tags, self.extra, variables)?;
                self.ads.apply_inputs_compiled(variables, now_ms)?;
                self.opcua.apply_inputs_compiled(variables, now_ms)?;
                // The program's `Comm.Link` blocks show their links before the program runs
                // (specification 42, §4).
                self.links.apply_link_states(
                    &mut Access(variables),
                    LinkSources {
                        drivers: &mut lock_drivers(self.drivers),
                        ads: &*self.ads,
                        opcua: &*self.opcua,
                        input_len: self.input_len,
                    },
                    i64::try_from(now_ms).unwrap_or(i64::MAX),
                );
                Ok(())
            }
            CompiledAccessPoint::BeforeOutputs => {
                capture_output_tags(self.tags, self.extra, variables)?;
                self.ads.capture_outputs_compiled(variables, now_ms)?;
                self.opcua.capture_outputs_compiled(variables, now_ms)
            }
            CompiledAccessPoint::CycleStart | CompiledAccessPoint::HoldEnd => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime_core::io_image::IoValueCodec;
    use trust_runtime_core::stdlib::conversions::ConversionType;
    use trust_runtime_core::value::Value;

    /// Specification 44 §13.3: `io.read` gives a binding's value the type of its variable, as a
    /// chip's snapshot does. The marker word `%MW0` of an `INT` holding the bits `0xFFFE` is the
    /// `INT` -2, not the `WORD` 65534. Catches a snapshot that reports the image's raw bits.
    #[test]
    fn the_io_snapshot_gives_a_binding_its_variables_type() {
        let bindings = [PreparedIoBinding {
            address: crate::io::IoAddress::parse("%MW0").expect("an address"),
            symbol: 0,
            codec: IoValueCodec {
                wire_type: Some(ConversionType::Int),
                ..IoValueCodec::default()
            },
        }];
        let names = [Some(SmolStr::new("Vat.RegState"))];
        let images = FlatImages {
            memory: vec![0xFE, 0xFF],
            ..FlatImages::default()
        };
        let snapshot = io_snapshot(&bindings, &names, 7, &images, &DebugControl::new());
        assert_eq!(snapshot.scan, Some(7));
        assert!(matches!(
            snapshot.memory[0].value,
            IoSnapshotValue::Value(Value::Int(-2))
        ));
    }
}
