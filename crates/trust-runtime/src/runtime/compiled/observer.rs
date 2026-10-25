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
use super::super::metrics_subsystem::MetricsSubsystem;
use super::super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::tags::{apply_input_tags, capture_output_tags, ExtraImages, TagLink};
use super::CompiledDebugger;
use crate::debug::{DebugControl, RuntimeEvent};
use crate::error::RuntimeError;
use crate::io::{IoSnapshot, IoSnapshotEntry, IoSnapshotValue};
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
    /// The process-image bindings and their declared names, for the I/O snapshot.
    pub(super) bindings: &'a [PreparedIoBinding],
    pub(super) binding_names: &'a [Option<SmolStr>],
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

    /// The I/O snapshot of the scan that ends: each binding's value read from the images.
    fn io_snapshot(&self, images: &FlatImages, debug: &DebugControl) -> IoSnapshot {
        let mut snapshot = IoSnapshot {
            scan: Some(self.cycle),
            forced: debug
                .forced_snapshot()
                .io
                .into_iter()
                .map(|(address, _)| address)
                .collect(),
            ..IoSnapshot::default()
        };
        for (binding, name) in self.bindings.iter().zip(self.binding_names) {
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
}

impl CycleHooks for HostedCompiledScan<'_> {
    fn on_compiled_phase(
        &mut self,
        phase: CyclePhase,
        now: Duration,
        images: &FlatImages,
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
                if let Some(debug) = &self.debug {
                    debug.push_io_snapshot(self.io_snapshot(images, debug));
                }
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
            debug.apply_compiled_live_values(point, self.generation, variables, now)?;
        }
        let now_ms = u64::try_from(now.as_millis()).unwrap_or(0);
        match point {
            CompiledAccessPoint::AfterInputs => {
                apply_input_tags(self.tags, self.extra, variables)?;
                self.ads.apply_inputs_compiled(variables, now_ms)?;
                self.opcua.apply_inputs_compiled(variables, now_ms)
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
        let metrics = MetricsSubsystem::new();
        let bindings = [PreparedIoBinding {
            address: crate::io::IoAddress::parse("%MW0").expect("an address"),
            symbol: 0,
            codec: IoValueCodec {
                wire_type: Some(ConversionType::Int),
                ..IoValueCodec::default()
            },
        }];
        let names = [Some(SmolStr::new("Vat.RegState"))];
        let mut ads = AdsSubsystem::new();
        let mut opcua = OpcUaClientSubsystem::new();
        let extra = Mutex::new(ExtraImages::default());
        let scan = HostedCompiledScan {
            metrics: &metrics,
            debug: None,
            debugger: None,
            ads: &mut ads,
            opcua: &mut opcua,
            tags: &[],
            extra: &extra,
            tasks: &[],
            programs: &[],
            bindings: &bindings,
            binding_names: &names,
            cycle: 7,
            generation: Generation::FIRST,
            task_timer: None,
            program_timer: None,
            output_deadline: None,
            held_at_start: std::time::Duration::ZERO,
        };
        let images = FlatImages {
            memory: vec![0xFE, 0xFF],
            ..FlatImages::default()
        };
        let snapshot = scan.io_snapshot(&images, &DebugControl::new());
        assert_eq!(snapshot.scan, Some(7));
        assert!(matches!(
            snapshot.memory[0].value,
            IoSnapshotValue::Value(Value::Int(-2))
        ));
    }
}
