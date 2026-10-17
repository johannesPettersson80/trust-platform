//! The installed program on the shared engine (specification 34, ARCH-11 and STATE-08): the
//! [`ResourceCycle`] of every device without an operating system, over the engine's
//! `RuntimeState`, and what the platform adapter supplies to it.
//!
//! - [`ProcessIo`]: the drivers of the process image. The cycle reads the inputs before the
//!   engine samples them, drives the outputs at the engine's publication point and the safe
//!   outputs of a stop or a fault, and reports the drivers' health in `status`.
//! - [`DeviceServices`]: the engine's execution services, with the scan deadline on the
//!   platform's monotonic clock and no wall clock until slice T6.
//! - [`device_limits`]: the admission limits of the device's application region and heap.
//!
//! trust-runtime on a computer runs the same engine through its own hosted cycle
//! (`runtime/installed_cycle.rs`); this cycle acts at the same phases and keeps the same output
//! and safe-state rules.

mod limits;
mod process_io;
mod services;

use alloc::vec::Vec;

use smol_str::SmolStr;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::io_address::IoAddress;
use trust_runtime_core::ports::{MonotonicClock, MonotonicInstant};
use trust_runtime_core::retain::{RestartMode, RetainSnapshot};
use trust_runtime_core::value::{Duration, Value};
use trust_runtime_core::vm::{
    CycleAccess, CycleHooks, CyclePhase, PreparedModule, RetainApplyEvent, RuntimeState,
};
use trust_runtime_core::watchdog::{FaultPolicy, WatchdogPolicy};

pub use limits::{device_limits, DeviceMemory};
pub use process_io::ProcessIo;
pub use services::DeviceServices;

use super::cycle::{resource_tasks, ResourceCycle, RetainSource, TaskDescriptor};
use crate::status::IoDriverStatus;

/// The only resource a source-free composition admits (spec 34 §5.2).
const RESOURCE_INDEX: usize = 0;

/// What the deployment configures for the installed program besides its code (specification
/// 34, §5.1 and TARGET-08).
#[derive(Debug, Clone)]
pub struct CycleSettings {
    /// The resource's cycle interval.
    pub cycle_interval: Duration,
    /// The cycle watchdog of `[runtime.watchdog]`.
    pub watchdog: WatchdogPolicy,
    /// What follows a failed scan, `[runtime.fault]`.
    pub fault_policy: FaultPolicy,
    /// The safe value of each output, `io.toml` `[[io.safe_state]]`, in its order. Without any,
    /// a stop or a fault drives no output, as on trust-runtime.
    pub safe_outputs: Vec<(IoAddress, Value)>,
}

/// The installed program of a device: resource 0 of a prepared module on the shared engine,
/// with the platform's process I/O.
///
/// `'a` is the life of the prepared module and the execution services, which the engine state
/// borrows; a firmware keeps both for the rest of the boot (`'static`). `C` is the platform's
/// monotonic clock, `I` its process I/O.
///
/// There is no retain store until slice C5: a stop saves nothing, and a warm restart keeps the
/// retained values the device holds in its memory.
pub struct EngineCycle<'a, C, I> {
    state: RuntimeState<'a>,
    prepared: &'a PreparedModule,
    services: &'a DeviceServices<C>,
    io: I,
    settings: CycleSettings,
}

impl<'a, C: MonotonicClock, I: ProcessIo> EngineCycle<'a, C, I> {
    /// Starts the program of `prepared` as power-on starts it: the engine constructs resource 0
    /// with `services`, which is a cold start. The resource loop then starts its task timing.
    ///
    /// # Errors
    ///
    /// The [`RuntimeError`] of a program the device cannot start, for example one that calls
    /// `CURRENT_DT` on a device without a wall clock; the device then serves the link without a
    /// program ([`Runner::stopped`](crate::runner::Runner::stopped)).
    pub fn start(
        prepared: &'a PreparedModule,
        services: &'a DeviceServices<C>,
        io: I,
        settings: CycleSettings,
    ) -> Result<Self, RuntimeError> {
        let state = prepared.instantiate_with_services(RESOURCE_INDEX, services)?;
        Ok(Self {
            state,
            prepared,
            services,
            io,
            settings,
        })
    }
}

impl<C: MonotonicClock, I: ProcessIo> ResourceCycle for EngineCycle<'_, C, I> {
    fn tasks(&self) -> Vec<TaskDescriptor> {
        resource_tasks(self.prepared, RESOURCE_INDEX)
    }

    fn cycle_interval(&self) -> Duration {
        self.settings.cycle_interval
    }

    fn watchdog(&self) -> WatchdogPolicy {
        self.settings.watchdog
    }

    fn fault_policy(&self) -> FaultPolicy {
        self.settings.fault_policy
    }

    /// The deadline is armed for this scan only, so a stop's or a fault's safe outputs are
    /// never refused by an expired one.
    fn run_scan(
        &mut self,
        now: Duration,
        deadline: Option<MonotonicInstant>,
        observer: &mut dyn CycleHooks,
    ) -> Result<(), RuntimeError> {
        let mut hooks = ScanHooks {
            io: &mut self.io,
            observer,
        };
        self.services.arm(deadline);
        let result = self.state.execute_cycle_with_hooks(now, &mut hooks);
        self.services.arm(None);
        result
    }

    fn latch_fault(&mut self, fault: RuntimeError) {
        self.state.latch_fault(fault);
    }

    /// As trust-runtime applies `[[io.safe_state]]`: each safe value is written into the
    /// engine's output image through its checked image access, and the drivers then drive that
    /// image.
    fn apply_safe_outputs(&mut self) -> Result<(), RuntimeError> {
        if self.settings.safe_outputs.is_empty() {
            return Ok(());
        }
        for (address, value) in &self.settings.safe_outputs {
            self.state.write_image(address, value.clone())?;
        }
        self.io.write_safe_outputs(self.state.outputs())
    }

    fn save_retain(&mut self) -> Result<(), RuntimeError> {
        Ok(())
    }

    /// A warm restart from the checkpoint loads the empty checkpoint of a device without a
    /// retain store, as trust-runtime loads it without one, and keeps the RETAIN variables the
    /// program holds in memory: after a stop, the values they had when it stopped (IEC 61131-3
    /// §6.5.6.1).
    fn restart(
        &mut self,
        mode: RestartMode,
        source: RetainSource,
    ) -> Result<Vec<RetainApplyEvent>, RuntimeError> {
        match (mode, source) {
            (RestartMode::Warm, RetainSource::Checkpoint) => self
                .state
                .restart_with_retain_snapshot(RestartMode::Warm, &RetainSnapshot::default()),
            (mode, RetainSource::Memory | RetainSource::Checkpoint) => {
                self.state.restart(mode)?;
                Ok(Vec::new())
            }
        }
    }

    fn reset_task_timing(&mut self, now: Duration) {
        self.state.reset_task_timing(now);
    }

    fn io_drivers(&self) -> Vec<IoDriverStatus> {
        self.io.driver_health()
    }
}

/// The engine hooks of one scan: the process I/O at its phases, and every callback to the
/// loop's observer.
///
/// They keep the engine's default `has_output_hooks`, true, so the engine checks the scan's
/// deadline before it publishes the outputs and restores the image it staged when the drivers
/// fail, as trust-runtime's hosted cycle does.
struct ScanHooks<'h, 'o, I> {
    io: &'h mut I,
    observer: &'h mut (dyn CycleHooks + 'o),
}

impl<I: ProcessIo> CycleHooks for ScanHooks<'_, '_, I> {
    fn has_program_hooks(&self) -> bool {
        self.observer.has_program_hooks()
    }

    fn program_started(&mut self, name: &SmolStr) {
        self.observer.program_started(name);
    }

    fn program_finished(&mut self, name: &SmolStr) {
        self.observer.program_finished(name);
    }

    /// The inputs are read at `BeforeInputSample`, where trust-runtime's drivers fill the input
    /// image, after the observer has seen the phase.
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        self.observer.on_phase(phase, access)?;
        if phase == CyclePhase::BeforeInputSample {
            self.io.read_inputs(access.inputs_mut()?)?;
        }
        Ok(())
    }

    fn publish_outputs(&mut self, access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        self.observer.publish_outputs(access)?;
        self.io.write_outputs(access.outputs())
    }
}

#[cfg(test)]
mod contract_tests;
