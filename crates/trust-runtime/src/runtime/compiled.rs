//! A compiled program on a computer (specification 44 §8.8, NX-14): the chips' compiled cycle over
//! the kernel's program, with the computer's ports, the drivers of `io.toml` ([`HostIo`]), its
//! monotonic clock and its retain file (the [`RetainManager`] the bytecode engine writes), and what
//! the scan thread asks of a program between and in its scans: its logical time, its scans and
//! restarts, its faults and the policies that follow them, its deadlines and safe outputs, its
//! online changes and stepped scans, and the state the other threads read.

mod clients;
mod debugger;
mod named;
mod observer;
mod process_io;
mod state;
mod tags;

use std::sync::{Arc, Mutex};
use std::time::{Duration as PhysicalDuration, Instant};

use smol_str::SmolStr;
use trust_runtime_core::io_image::PreparedIoBinding;
use trust_runtime_core::native::live::io_bindings;
use trust_runtime_core::native::program::AdmittedImage;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant, TimeSpan};
use trust_runtime_services::cycle_step::{StepOutcome, StepRefusal, StepRequest};
use trust_runtime_services::runner::engine::CycleSettings;
use trust_runtime_services::runner::{CompiledCycle, ResourceCycle, RetainSource, TaskDescriptor};

use super::ads_subsystem::AdsSubsystem;
use super::metrics_subsystem::MetricsSubsystem;
use super::opcua_client_subsystem::OpcUaClientSubsystem;
use super::openot_telemetry::OpenOtTelemetrySubsystem;
use super::watchdog_subsystem::WatchdogSubsystem;
use crate::debug::{DebugControl, RuntimeEvent};
use crate::error::RuntimeError;
use crate::execution_backend::ExecutionBackend;
use crate::io::{IoAddress, IoDriver, IoDriverStatus, IoSafeState};
use crate::memory::IoArea;
use crate::metrics::{HostMonotonicClock, RuntimeMetrics};
use crate::native::HostCode;
use crate::retain::{RetainManager, RetainStore};
use crate::value::{Duration, Value};
use crate::watchdog::{FaultAction, FaultDecision, FaultPolicy, WatchdogPolicy};
use crate::RestartMode;

pub use debugger::CompiledDebugger;
use debugger::HostHold;
pub use named::NamedVariable;
pub(crate) use process_io::HostIo;
pub use state::{CompiledState, CompiledVariable};

/// The cycle of a compiled program on a computer.
pub(crate) type HostCycle =
    CompiledCycle<'static, HostMonotonicClock, HostIo, HostCode, HostHold, RetainManager>;

/// A compiled program installed on a computer, with the computer's adapters around it.
pub struct CompiledRuntime {
    cycle: HostCycle,
    /// The admitted image the program keeps, which the debugger reads a held scan through.
    image: Arc<AdmittedImage>,
    resource_name: SmolStr,
    /// The program's tasks, in the image's order without the programs that have no task.
    tasks: Vec<TaskDescriptor>,
    /// The image's process-image bindings and their declared names, for the I/O snapshot.
    bindings: Vec<PreparedIoBinding>,
    binding_names: Vec<Option<SmolStr>>,
    /// The cycle's clock, on which a deadline is given to it.
    clock: HostMonotonicClock,
    metrics: MetricsSubsystem,
    watchdog: WatchdogSubsystem,
    fault_policy: FaultPolicy,
    /// The ADS and OPC UA clients of the configuration, whose points bind the program's globals.
    ads: AdsSubsystem,
    opcua_client: OpcUaClientSubsystem,
    /// The OpenOT telemetry, which publishes after each scan.
    openot_telemetry: OpenOtTelemetrySubsystem,
    /// The MQTT tags past the program's own process image, and the bytes they lie in, which the
    /// process I/O shares.
    tags: Vec<tags::TagLink>,
    extra: tags::SharedExtraImages,
    debug: Option<DebugControl>,
    /// The debugger of the program, once a debug control is set (specification 44 §13.2, §8.8).
    debugger: Option<CompiledDebugger>,
    current_time: Duration,
    cycle_counter: u64,
    execution_deadline: Option<Instant>,
    output_commit_deadline: Option<Instant>,
    output_commit_pause_baseline: PhysicalDuration,
    /// The statements and loop passes one task activation runs at most (specification 44 NX-10),
    /// for this program and every program an online change installs.
    statement_budget: u32,
}

/// This computer's device for a program whose task activations run at most `budget` statements.
fn computer(budget: u32) -> Result<crate::native::Device, RuntimeError> {
    let mut device = crate::native::this_computer().ok_or_else(|| {
        RuntimeError::InvalidBundle("this computer's CPU runs no program image".into())
    })?;
    device.max_statements = budget;
    Ok(device)
}

impl std::fmt::Debug for CompiledRuntime {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("CompiledRuntime")
            .field("resource_name", &self.resource_name)
            .field("cycle_counter", &self.cycle_counter)
            .field("fault", &self.cycle.fault())
            .finish_non_exhaustive()
    }
}

impl CompiledRuntime {
    /// The compiled program of the image `bytes` on this computer, admitted against the
    /// computer's device (specification 44 §10.2, NX-15) and not started: the scan thread restarts
    /// it, cold, or warm from its retain file, once its adapters are set. Each task activation runs
    /// at most 1,000,000 statements and loop passes, the default of NX-10.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidBundle`] for an image the computer refuses, with the refusal's text.
    pub fn load(bytes: Vec<u8>, resource_name: impl Into<SmolStr>) -> Result<Self, RuntimeError> {
        Self::load_with_statement_budget(
            bytes,
            resource_name,
            trust_runtime_core::deployment::HOSTED_MAX_STATEMENTS,
        )
    }

    /// [`Self::load`] with the statement budget `budget` of each task activation, the project's
    /// `[resource] max_statements_per_scan` (specification 44 NX-10).
    ///
    /// # Errors
    ///
    /// As [`Self::load`].
    pub fn load_with_statement_budget(
        bytes: Vec<u8>,
        resource_name: impl Into<SmolStr>,
        budget: u32,
    ) -> Result<Self, RuntimeError> {
        let device = computer(budget)?;
        let program = crate::native::load_kept(bytes, &device)
            .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
        let (bindings, binding_names) = io_bindings(&program.image())
            .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
        let image = program
            .kept_image()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        let clock = HostMonotonicClock::default();
        let settings = CycleSettings {
            cycle_interval: Duration::ZERO,
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::Halt,
            safe_outputs: Vec::new(),
        };
        let io = HostIo::default();
        let extra = io.extra_images();
        let cycle = HostCycle::unstarted(program, io, settings, clock);
        let tasks = cycle.tasks();
        let runtime = Self {
            cycle,
            image,
            resource_name: resource_name.into(),
            tasks,
            bindings,
            binding_names,
            clock,
            metrics: MetricsSubsystem::new(),
            watchdog: WatchdogSubsystem::default(),
            fault_policy: FaultPolicy::Halt,
            ads: AdsSubsystem::new(),
            opcua_client: OpcUaClientSubsystem::new(),
            openot_telemetry: OpenOtTelemetrySubsystem::default(),
            tags: Vec::new(),
            extra,
            debug: None,
            debugger: None,
            current_time: Duration::ZERO,
            cycle_counter: 0,
            execution_deadline: None,
            output_commit_deadline: None,
            output_commit_pause_baseline: PhysicalDuration::ZERO,
            statement_budget: budget,
        };
        runtime.publish_program_memory();
        Ok(runtime)
    }

    /// The resource's name.
    #[must_use]
    pub fn resource_name(&self) -> &SmolStr {
        &self.resource_name
    }

    /// The program's tasks, in the image's order.
    #[must_use]
    pub fn tasks(&self) -> &[TaskDescriptor] {
        &self.tasks
    }

    /// The backend `status` reports: a compiled program.
    #[must_use]
    pub fn execution_backend(&self) -> ExecutionBackend {
        ExecutionBackend::Native
    }

    /// The program's logical time.
    #[must_use]
    pub fn current_time(&self) -> Duration {
        self.current_time
    }

    /// Sets the logical time of the next scan.
    pub fn set_current_time(&mut self, now: Duration) {
        self.current_time = now;
    }

    /// Every task counts its next activation from `now`, which becomes the program's time.
    pub fn reset_task_timing(&mut self, now: Duration) {
        self.cycle.reset_task_timing(now);
        self.current_time = now;
    }

    /// The scans completed since the last restart.
    #[must_use]
    pub fn cycle_counter(&self) -> u64 {
        self.cycle_counter
    }

    /// Runs one scan at the program's logical time: the compiled cycle with the computer's
    /// observer, then the OpenOT telemetry and the output commit deadline, the drivers' health, the
    /// cycle's end event and its metrics. A failed scan latches its fault and applies the fault
    /// policy's output action (specification 11 §6.6).
    ///
    /// # Errors
    ///
    /// [`RuntimeError::ResourceFaulted`] while a fault is latched; the scan's error otherwise, after
    /// the safe outputs a halt drives.
    pub fn execute_cycle(&mut self) -> Result<(), RuntimeError> {
        if self.faulted() {
            return Err(RuntimeError::ResourceFaulted);
        }
        let now = self.current_time;
        let cycle_timer = self.metrics.start_timer();
        let deadline = self
            .execution_deadline
            .map(|deadline| self.on_clock(deadline));
        let generation = self.cycle.generation();
        let mut observer = observer::HostedCompiledScan {
            metrics: &self.metrics,
            debug: self.debug.clone(),
            ads: &mut self.ads,
            opcua: &mut self.opcua_client,
            tags: &self.tags,
            extra: &self.extra,
            tasks: &self.tasks,
            bindings: &self.bindings,
            binding_names: &self.binding_names,
            cycle: self.cycle_counter,
            generation,
            task_timer: None,
            program_timer: None,
        };
        let mut result = self.cycle.run_scan(now, deadline, &mut observer);
        // The telemetry publishes after the outputs reached the drivers, as on the bytecode
        // engine: a late failure latches a fault, but does not undo the outputs.
        if result.is_ok() {
            result = self
                .openot_telemetry
                .publish_compiled(self.cycle_counter, self.cycle.program())
                .and_then(|()| {
                    if self
                        .effective_output_commit_deadline()
                        .is_some_and(|deadline| Instant::now() >= deadline)
                    {
                        Err(RuntimeError::WatchdogTimeout)
                    } else {
                        Ok(())
                    }
                });
        }
        self.cycle.process_io_mut().update_health();
        match result {
            Ok(()) => {
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::CycleEnd {
                        cycle: self.cycle_counter,
                        time: now,
                    });
                }
                // The state the reads between scans answer from (specification 11 §6.13.1).
                if let Some(debugger) = &self.debugger {
                    let program = self.cycle.program();
                    debugger.publish(|| CompiledState::of(program, now).ok());
                }
                if let Some(start) = cycle_timer {
                    self.metrics.record_cycle(start.elapsed());
                }
                self.cycle_counter = self.cycle_counter.saturating_add(1);
                Ok(())
            }
            Err(error) => {
                Err(self.apply_fault(error, FaultDecision::from_fault_policy(self.fault_policy)))
            }
        }
    }

    /// `deadline` on the cycle's clock; a deadline that has passed is now.
    fn on_clock(&self, deadline: Instant) -> MonotonicInstant {
        let from_now = deadline.saturating_duration_since(Instant::now());
        self.clock.now().saturating_add(TimeSpan::new(from_now))
    }

    /// One stepped scan (`cycle.step`, specification 11 §6.8.1), as the compiled cycle runs it; the
    /// watchdog bounds it as it bounds every scan.
    ///
    /// # Errors
    ///
    /// The [`StepRefusal`] of a request the program does not take.
    pub fn step(
        &mut self,
        request: &StepRequest,
        restart_at: Duration,
    ) -> Result<StepOutcome, StepRefusal> {
        let watchdog = self.watchdog_policy();
        let deadline = watchdog.enabled.then(|| {
            let timeout = u64::try_from(watchdog.timeout.as_nanos()).unwrap_or(0);
            self.clock
                .now()
                .saturating_add(TimeSpan::new(PhysicalDuration::from_nanos(timeout)))
        });
        let outcome = self.cycle.step(request, restart_at, deadline)?;
        self.current_time = request.time;
        Ok(outcome)
    }

    /// Restarts the program in place: cold with its starting memory, warm with the retained values
    /// it holds (IEC 61131-3 §6.5.6.1).
    ///
    /// # Errors
    ///
    /// The fault of the restart; the program is then as it was.
    pub fn restart(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.restart_from(mode, RetainSource::Memory)
    }

    /// Restarts the program, warm with the values of its retain file, cold with its starting
    /// memory (specification 44 §11).
    ///
    /// # Errors
    ///
    /// The fault of the restart or the error of a retain file that does not read; the program is
    /// then as it was.
    pub fn restart_with_retain_store(&mut self, mode: RestartMode) -> Result<(), RuntimeError> {
        self.restart_from(mode, RetainSource::Checkpoint)
    }

    fn restart_from(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<(), RuntimeError> {
        let events = self.cycle.restart(mode, source)?;
        self.cycle_counter = 0;
        self.openot_telemetry.reset_scan_state();
        if let Some(debugger) = &self.debugger {
            debugger.restarted();
        }
        if let Some(debug) = &self.debug {
            debug.begin_lifecycle(self.cycle.generation());
            for event in events {
                debug.push_runtime_event(trust_runtime_services::events::retain_apply_event(
                    event,
                    self.current_time,
                ));
            }
        }
        self.publish_program_memory();
        Ok(())
    }

    /// Changes the running program online to the program of the image `bytes` (specification 44
    /// §12): the new program takes the running program's values and task schedule, the drivers and
    /// the retain file at the commit, and the running program is dropped. A refusal leaves the
    /// running program as it was.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidBundle`] for an image the computer refuses, and the error of a change
    /// that cannot start.
    pub fn apply_online_change_bytes(&mut self, bytes: &[u8]) -> Result<(), RuntimeError> {
        let device = computer(self.statement_budget)?;
        let program = crate::native::load_kept(bytes.to_vec(), &device)
            .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
        let (bindings, binding_names) = io_bindings(&program.image())
            .map_err(|error| RuntimeError::InvalidBundle(error.to_string().into()))?;
        let image = program
            .kept_image()
            .ok_or(RuntimeError::InvalidExecutionState)?;
        // The MQTT tags lie past the process image at the offsets their drivers were given.
        if !self.tags.is_empty()
            && [IoArea::Input, IoArea::Output].into_iter().any(|area| {
                tags::image_bytes(&image.image(), area) != self.process_image_bytes(area)
            })
        {
            return Err(RuntimeError::InvalidConfig(
                "the changed program's process image has another size, and MQTT tags lie past it: \
                 restart the runtime with the changed program"
                    .into(),
            ));
        }
        let mut candidate = HostCycle::candidate(program, HostIo::default(), &self.cycle);
        if let Some(debugger) = &self.debugger {
            candidate.attach_compiled_hold(debugger.hold());
        }
        candidate.check_safe_outputs()?;
        let payload = candidate.program().retain_payload_bound()?;
        self.cycle.reserve_retain(payload)?;
        let events = candidate.take_values(&self.cycle)?;
        self.cycle.hand_over_to(&mut candidate);
        self.cycle = candidate;
        self.image = image;
        if let Some(debugger) = &self.debugger {
            debugger.set_program(Arc::clone(&self.image));
        }
        self.tasks = self.cycle.tasks();
        self.bindings = bindings;
        self.binding_names = binding_names;
        if let Some(debug) = &self.debug {
            debug.begin_lifecycle(self.cycle.generation());
            for event in events {
                debug.push_runtime_event(RuntimeEvent::OnlineChangeValue {
                    event,
                    time: self.current_time,
                });
            }
        }
        self.publish_program_memory();
        Ok(())
    }

    /// Whether a fault is latched.
    #[must_use]
    pub fn faulted(&self) -> bool {
        self.cycle.fault().is_some()
    }

    /// The fault latched since the last restart.
    #[must_use]
    pub fn last_fault(&self) -> Option<RuntimeError> {
        self.cycle.fault().cloned()
    }

    /// Latches `error`, applies `decision`'s output action and records the fault.
    fn apply_fault(&mut self, error: RuntimeError, decision: FaultDecision) -> RuntimeError {
        if let Some(debug) = &self.debug {
            debug.clear_runtime_mutations();
        }
        self.cycle.latch_fault(error.clone());
        let mut reported = error.clone();
        if decision.apply_safe_state
            && let Err(safe_error) = self.apply_io_safe_state()
        {
            reported = RuntimeError::SafeStateFailed {
                root: error.to_string().into(),
                error: safe_error.to_string().into(),
            };
            if let Some(debug) = &self.debug {
                debug.push_runtime_event(RuntimeEvent::SafeStateFailed {
                    root: error.to_string(),
                    error: safe_error.to_string(),
                    time: self.current_time,
                });
            }
        }
        self.metrics.record_fault();
        if let Some(debug) = &self.debug {
            debug.push_runtime_event(RuntimeEvent::fault(&error, self.current_time));
        }
        reported
    }

    /// Reports a physical watchdog expiration through the common fault path.
    pub fn watchdog_timeout(&mut self) -> RuntimeError {
        self.watchdog_fault(RuntimeError::WatchdogTimeout)
    }

    /// Applies the watchdog's action for `error`.
    pub(crate) fn watchdog_fault(&mut self, error: RuntimeError) -> RuntimeError {
        self.apply_fault(error, self.watchdog.decision())
    }

    /// Reports a panic of the scan thread, halting with the safe outputs.
    pub fn resource_panic(&mut self, message: impl Into<SmolStr>) -> RuntimeError {
        self.apply_fault(
            RuntimeError::ResourcePanic(message.into()),
            FaultDecision {
                action: FaultAction::Halt,
                apply_safe_state: true,
            },
        )
    }

    /// Reports a simulation fault with the fault policy.
    pub fn simulation_fault(&mut self, message: impl Into<SmolStr>) -> RuntimeError {
        self.apply_fault(
            RuntimeError::SimulationFault(message.into()),
            FaultDecision::from_fault_policy(self.fault_policy),
        )
    }

    /// Drives the safe outputs of `[[io.safe_state]]`: each safe value into the output image, and
    /// every driver from it (specification 34 STATE-02).
    ///
    /// # Errors
    ///
    /// The first failure among the drivers.
    pub fn apply_io_safe_state(&mut self) -> Result<(), RuntimeError> {
        if let Some(debug) = &self.debug {
            debug.clear_runtime_mutations();
        }
        self.cycle.apply_safe_outputs()
    }

    /// Takes the safe outputs of `[[io.safe_state]]`.
    pub fn set_io_safe_state(&mut self, state: IoSafeState) {
        self.cycle.settings_mut().safe_outputs = state.outputs;
    }

    /// Saves the retained values into the retain file before the resource reports Stopped.
    ///
    /// # Errors
    ///
    /// The error of a save that did not complete.
    pub fn save_retain_store(&mut self) -> Result<(), RuntimeError> {
        self.cycle.save_retain()
    }

    /// Keeps the retained values in `store` from now on, saved at most every `interval`.
    ///
    /// # Errors
    ///
    /// The error of a retain map that does not read.
    pub fn set_retain_store(
        &mut self,
        store: Option<Box<dyn RetainStore>>,
        interval: Option<Duration>,
    ) -> Result<(), RuntimeError> {
        let mut manager = RetainManager::default();
        manager.configure(store, interval, self.current_time);
        self.cycle.set_retain_store(manager)?;
        self.publish_program_memory();
        Ok(())
    }

    /// Saves the retained values at most every `interval` from now on.
    pub fn set_retain_save_interval(&mut self, interval: Option<Duration>) {
        if let Some(retain) = self.cycle.retain_store_mut() {
            retain.set_save_interval(interval);
        }
    }

    /// The retain file's save interval.
    #[must_use]
    pub fn retain_save_interval(&self) -> Option<Duration> {
        self.cycle
            .retain_store()
            .and_then(RetainManager::save_interval)
    }

    /// The watchdog policy.
    #[must_use]
    pub fn watchdog_policy(&self) -> WatchdogPolicy {
        self.watchdog.policy()
    }

    /// Takes the watchdog policy of `[runtime.watchdog]`.
    pub fn set_watchdog_policy(&mut self, policy: WatchdogPolicy) {
        self.watchdog.set_policy(policy);
        self.cycle.settings_mut().watchdog = policy;
    }

    /// The fault policy.
    #[must_use]
    pub fn fault_policy(&self) -> FaultPolicy {
        self.fault_policy
    }

    /// Takes the fault policy of `[runtime.fault]`.
    pub fn set_fault_policy(&mut self, policy: FaultPolicy) {
        self.fault_policy = policy;
        self.cycle.settings_mut().fault_policy = policy;
    }

    /// Sets the deadline of the next scan's execution.
    pub(crate) fn set_execution_deadline(&mut self, deadline: Option<Instant>) {
        self.execution_deadline = deadline;
    }

    /// The deadline of the next scan's execution.
    #[cfg(test)]
    #[must_use]
    pub(crate) fn execution_deadline(&self) -> Option<Instant> {
        self.execution_deadline
    }

    /// The deadline of the next scan's execution; the debugger's holds move it inside the cycle.
    #[must_use]
    pub(crate) fn effective_execution_deadline(&self) -> Option<Instant> {
        self.execution_deadline
    }

    /// Sets the deadline of the next scan's outputs.
    pub(crate) fn set_output_commit_deadline(&mut self, deadline: Option<Instant>) {
        self.output_commit_deadline = deadline;
        self.output_commit_pause_baseline = self.debug_watchdog_pause_elapsed();
    }

    #[cfg(test)]
    pub(crate) fn output_commit_deadline(&self) -> Option<Instant> {
        self.output_commit_deadline
    }

    /// The deadline of the next scan's outputs, moved by the debugger's pauses since it was set.
    #[must_use]
    pub(crate) fn effective_output_commit_deadline(&self) -> Option<Instant> {
        self.output_commit_deadline.map(|deadline| {
            deadline
                .checked_add(
                    self.debug_watchdog_pause_elapsed()
                        .saturating_sub(self.output_commit_pause_baseline),
                )
                .unwrap_or(deadline)
        })
    }

    /// The time the debugger held scans in total, which no watchdog counts.
    #[must_use]
    pub fn debug_watchdog_pause_elapsed(&self) -> PhysicalDuration {
        self.debug
            .as_ref()
            .map_or(PhysicalDuration::ZERO, DebugControl::watchdog_pause_elapsed)
    }

    /// Takes the debugger's control, and with it the debugger of compiled programs, which holds
    /// the program's scans from now on (specification 44 §13.2, §8.8).
    pub fn set_debug_control(&mut self, debug: DebugControl) {
        debug.begin_lifecycle(self.cycle.generation());
        let debugger = debug.compiled_debugger();
        debugger.set_program(Arc::clone(&self.image));
        self.cycle.attach_compiled_hold(debugger.hold());
        self.debugger = Some(debugger);
        self.debug = Some(debug);
    }

    /// The debugger's control, when one is set.
    #[must_use]
    pub fn debug_control(&self) -> Option<DebugControl> {
        self.debug.clone()
    }

    /// The status of the configuration's ADS connections.
    #[must_use]
    pub fn ads_status_report(&self) -> crate::ads::diagnostics::AdsStatusReport {
        self.ads.status_report()
    }

    /// The status of the configuration's OPC UA client connections.
    #[must_use]
    pub fn opcua_client_status_report(&self) -> crate::opcua::OpcUaClientStatusReport {
        self.opcua_client.status_report()
    }

    /// The live ADS device that overlaps a doctor target.
    #[must_use]
    pub fn active_ads_device_snapshot(
        &self,
        target: &crate::ads::diagnostics::TargetIdentity,
        local: Option<&crate::ads::diagnostics::LocalIdentity>,
    ) -> Option<crate::ads::onboarding::ActiveAdsDeviceSnapshot> {
        self.ads.active_device_snapshot(target, local)
    }

    /// Records the metrics into `metrics` from now on.
    pub fn set_metrics_sink(&mut self, metrics: Arc<Mutex<RuntimeMetrics>>) {
        self.metrics.set_sink(metrics);
        self.metrics.set_execution_backend(ExecutionBackend::Native);
        self.publish_program_memory();
    }

    /// Gives the metrics the memory figure of the program, which `status` reports (specification
    /// 34 MEM-12; specification 44 §10.4).
    fn publish_program_memory(&self) {
        if let Some(memory) = self.cycle.program_memory() {
            self.metrics.set_program_memory(memory);
        }
    }

    /// Adds the `io.toml` driver `driver` named `name` after the others.
    pub fn add_io_driver(&mut self, name: impl Into<SmolStr>, driver: Box<dyn IoDriver>) {
        self.cycle.process_io_mut().add_driver(name, driver);
    }

    /// Takes `io`, a project's `io.toml`, as trust-runtime starts a compiled program
    /// (specification 44 §8.8): its safe outputs, and each enabled driver after the others, an MQTT
    /// driver with its tag mappings bound to the program's variables first (§13.3).
    ///
    /// # Errors
    ///
    /// The error of a tag mapping or a driver that does not build.
    pub fn add_io_config(&mut self, io: &crate::config::IoConfig) -> Result<(), RuntimeError> {
        self.set_io_safe_state(io.safe_state.clone());
        let registry = crate::io::IoDriverRegistry::default_registry();
        for driver in io.drivers.iter().filter(|driver| driver.enabled) {
            let mut params = driver.params.clone();
            if driver.name.eq_ignore_ascii_case("mqtt")
                || driver.name.eq_ignore_ascii_case("mqtt-tcp")
            {
                let (lowered, links) =
                    crate::io::resolve_compiled_mqtt_tag_mappings(self, &driver.params)?;
                self.link_mqtt_tags(&links)?;
                params = lowered;
            }
            if let Some(spec) = registry.build(driver.name.as_str(), &params)? {
                self.add_io_driver(spec.name, spec.driver);
            }
        }
        Ok(())
    }

    /// Stops using every driver.
    pub fn clear_io_drivers(&mut self) {
        self.cycle.process_io_mut().clear_drivers();
    }

    /// Publishes the drivers' health into `sink` from now on.
    pub fn set_io_health_sink(&mut self, sink: Option<Arc<Mutex<Vec<IoDriverStatus>>>>) {
        self.cycle.process_io_mut().set_health_sink(sink);
    }

    /// Writes `value` at the process-image `address` between scans, as a simulation writes its
    /// inputs.
    ///
    /// # Errors
    ///
    /// The refusal of an address or a value the image does not take.
    pub fn write_io(&mut self, address: &IoAddress, value: Value) -> Result<(), RuntimeError> {
        self.cycle.program_mut().write_address(address, value)
    }

    /// The value at the process-image `address`, as the last scan left it.
    ///
    /// # Errors
    ///
    /// The refusal of an address the image does not hold.
    pub fn read_io(&self, address: &IoAddress) -> Result<Value, RuntimeError> {
        self.cycle.program().read_address(address)
    }

    /// The program's state as the last scan left it, for the other threads (specification 44
    /// §8.8): a copy of its areas with its image.
    ///
    /// # Errors
    ///
    /// [`RuntimeError::InvalidExecutionState`] for a program that does not keep its image.
    pub fn snapshot(&self) -> Result<CompiledState, RuntimeError> {
        CompiledState::of(self.cycle.program(), self.current_time)
    }

    /// The values of the variables `names` names, as the test harness resolves a name: a global, a
    /// program's variable `Program.var`, or a unique unqualified name; a name that names no single
    /// variable is left out.
    #[must_use]
    pub fn snapshot_globals(&self, names: &[SmolStr]) -> indexmap::IndexMap<SmolStr, Value> {
        let program = self.cycle.program();
        let image = program.image();
        names
            .iter()
            .filter_map(|name| {
                let symbol = image.find_variable(name).ok()?;
                let value = program.read_symbol(symbol).ok()?;
                Some((name.clone(), value))
            })
            .collect()
    }

    /// Writes each of `updates` into the variable its name names, between scans, as a mesh sample
    /// arrives; a write the program refuses is reported as `write_rejected`.
    pub fn apply_mesh_updates(&mut self, updates: &indexmap::IndexMap<SmolStr, Value>) {
        for (name, value) in updates {
            let program = self.cycle.program_mut();
            let symbol = program.image().find_variable(name);
            let result = match symbol {
                Ok(symbol) => program.write_symbol(symbol, value),
                Err(_) => Err(RuntimeError::UndefinedVariable(name.clone())),
            };
            if let Err(error) = result
                && let Some(debug) = &self.debug
            {
                debug.push_runtime_event(RuntimeEvent::WriteRejected {
                    target: name.clone(),
                    error: error.to_string(),
                    time: self.current_time,
                });
            }
        }
    }
}
