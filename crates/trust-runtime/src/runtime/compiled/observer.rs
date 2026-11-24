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
use trust_runtime_core::native::program::RustPouReport;
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
use trust_runtime_core::trace::readings;
use trust_runtime_core::trace::ChangeSource;
use trust_runtime_services::trace::{Begin, Recorder};

use super::trace::TraceSession;

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
    /// The cycle trace being recorded, which the scan's phases and the writes at its start fill
    /// (specification 48 RU-59).
    pub(super) trace: Option<&'a mut TraceSession>,
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
        images: &FlatImages,
    ) -> Result<(), RuntimeError> {
        self.record_phase(phase, now, images);
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
        // A recording's start state is the program's memory before the cycle's writes
        // (specification 48 RU-59).
        if point == CompiledAccessPoint::CycleStart {
            take_start(self.trace.as_deref_mut(), variables);
        }
        if let Some(debug) = &self.debug {
            // Control Studio's experiment writes of this scan, before the Live Values, which a
            // force then holds over them (specification 35 A3.5).
            if point == CompiledAccessPoint::CycleStart {
                debug.apply_control_writes(&mut Access(variables), self.generation, now);
            }
            let recorder = self
                .trace
                .as_deref_mut()
                .map(|session| &mut session.recorder);
            debug.apply_compiled_live_values(point, self.generation, variables, now, recorder)?;
        }
        if point == CompiledAccessPoint::CycleStart {
            take_forces(self.trace.as_deref_mut(), self.debug.as_ref(), variables);
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
                // A recording keeps the input image as the scan samples it, once everything that
                // writes it before the program has (specification 48 RU-59).
                if let Some(session) = self.trace.as_deref_mut() {
                    session.recorder.inputs(variables.input_image());
                }
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

    fn on_rust_times_fresh(&mut self) {
        self.metrics.clear_rust_pous();
    }

    fn on_rust_pou(&mut self, report: &RustPouReport<'_>, now: Duration) {
        self.metrics.record_rust_pou(report);
        if report.exceeded
            && let Some(debug) = &self.debug
        {
            debug.push_runtime_event(RuntimeEvent::RustBudgetExceeded {
                name: SmolStr::new(report.name),
                call_ns: report.record.longest_ns,
                budget_ns: report.record.budget_ns,
                time: now,
            });
        }
    }
}

impl HostedCompiledScan<'_> {
    /// The cycle trace's part of a phase (specification 48 RU-59): the cycle's start, each task's
    /// activation, the input image as the scan sampled it, the output image it wrote, and the
    /// cycle's end with the wall clock's readings.
    fn record_phase(&mut self, phase: CyclePhase, now: Duration, images: &FlatImages) {
        let Some(session) = self.trace.as_deref_mut() else {
            return;
        };
        let recorder = &mut session.recorder;
        match phase {
            CyclePhase::CycleStart => {
                if recorder.begin_cycle(now, self.cycle) == Begin::First {
                    session.start_pending = true;
                }
                readings::arm();
            }
            CyclePhase::TaskStart(index) => {
                recorder.activation(u32::try_from(index).unwrap_or(u32::MAX), now.as_nanos());
            }
            // The output image is the scan's own here (`CycleHooks::on_compiled_phase`).
            CyclePhase::AfterOutputSample => recorder.outputs(&images.outputs),
            CyclePhase::CycleEnd => {
                take_readings(recorder);
                recorder.end_cycle(None);
            }
            _ => {}
        }
    }
}

/// The start state of a recording's first cycle: each area before the cycle's writes.
fn take_start(session: Option<&mut TraceSession>, variables: &dyn CompiledVariables) {
    let Some(session) = session.filter(|session| session.start_pending) else {
        return;
    };
    for id in 1..=6 {
        if let Some(area) = session.recorder.start_area(id) {
            variables.read_area_bytes(id, 0, area);
        }
    }
}

/// The forces in effect at a recording's first cycle, once the cycle's start applied them: each
/// variable by its path, a member of an instance included, and each address by the bytes it gives
/// the process image. The start state is then whole.
fn take_forces(
    session: Option<&mut TraceSession>,
    debug: Option<&DebugControl>,
    variables: &dyn CompiledVariables,
) {
    let Some(session) = session.filter(|session| session.start_pending) else {
        return;
    };
    if let Some(debug) = debug {
        let forces = debug.forced_snapshot();
        session.recorder.start_forces(
            ChangeSource::ControlLink,
            &forces.vars,
            &forces.io,
            variables,
        );
    }
    session.start_pending = false;
}

/// Hands the wall clock's readings of the scan to `recorder` and stops keeping them: readings the
/// room could not keep end the recording `memory_full`.
pub(super) fn take_readings(recorder: &mut Recorder) {
    let mut handed = 0;
    let read = readings::take(&mut |reading| {
        recorder.wall_clock(reading);
        handed += 1;
    });
    if read > handed {
        recorder.readings_lost();
    }
    readings::disarm();
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
