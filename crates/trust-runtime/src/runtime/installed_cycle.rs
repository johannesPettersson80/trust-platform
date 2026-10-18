//! Host adapters at the shared scheduler's established transaction boundaries.

use super::{
    ads_subsystem::AdsSubsystem, io_subsystem::IoSubsystem, metrics_subsystem::MetricsSubsystem,
    opcua_client_subsystem::OpcUaClientSubsystem, RetainSnapshot, Runtime, RuntimeMetadata,
};
use crate::{
    debug::{DebugControl, RuntimeEvent},
    error::RuntimeError,
    program_model::{Expr, LValue},
    task::TaskConfig,
    value::Value,
    watchdog::{FaultAction, FaultDecision},
};
use std::time::{Duration as PhysicalDuration, Instant};
use trust_runtime_core::vm::{CycleAccess, CycleHooks, CyclePhase};

struct HostedCycle<'a> {
    io: &'a mut IoSubsystem,
    ads: &'a mut AdsSubsystem,
    opcua: &'a mut OpcUaClientSubsystem,
    retain: &'a mut crate::retain::RetainManager,
    metrics: &'a MetricsSubsystem,
    metadata: &'a RuntimeMetadata,
    tasks: &'a [TaskConfig],
    debug: Option<DebugControl>,
    cycle: u64,
    task_timer: Option<Instant>,
    program_timer: Option<Instant>,
    output_deadline: Option<Instant>,
    output_pause_baseline: PhysicalDuration,
}

impl Runtime {
    /// Execute the shared cooperative scheduler with borrowed hosted services.
    pub fn execute_cycle(&mut self) -> Result<(), RuntimeError> {
        if self.faulted() {
            return Err(RuntimeError::ResourceFaulted);
        }
        let debug = self.debug_control();
        let now = self.current_time();
        let cycle_timer = self.metrics.start_timer();
        let mut result = {
            let mut hooks = HostedCycle {
                io: &mut self.io,
                ads: &mut self.ads,
                opcua: &mut self.opcua_client,
                retain: &mut self.retain,
                metrics: &self.metrics,
                metadata: &self.metadata,
                tasks: &self.tasks,
                debug,
                cycle: self.cycle_counter,
                task_timer: None,
                program_timer: None,
                output_deadline: self.output_commit_deadline,
                output_pause_baseline: self.output_commit_pause_baseline,
            };
            self.state.execute_cycle_with_hooks(now, &mut hooks)
        };
        // The old hosted transaction committed flat Q before telemetry and its
        // final deadline check. Keep that boundary: a late telemetry failure
        // latches a fault, but does not undo already published driver outputs.
        if result.is_ok() {
            result = self
                .openot_telemetry
                .publish(self.cycle_counter, self.state.storage())
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
            self.io.update_health();
        }
        match result {
            Ok(()) => {
                if let Some(debug) = self.debug_control() {
                    debug.push_runtime_event(RuntimeEvent::CycleEnd {
                        cycle: self.cycle_counter,
                        time: now,
                    });
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

    /// Apply configured safe output policy even after an execution deadline has
    /// expired, attempting all drivers while preserving the owning PLC fault.
    pub fn apply_io_safe_state(&mut self) -> Result<(), RuntimeError> {
        if let Some(debug) = self.debug_control() {
            debug.clear_runtime_mutations();
        }
        // Safe-output delivery is a separate owner entry. Expiry must not block
        // the very safety action requested by watchdog/fault policy.
        let deadline = self.state.services().suspend_execution_deadline();
        let result = self.io.apply_safe_state_installed(&mut self.state);
        self.state.services().restore_execution_deadline(deadline);
        result
    }

    /// Fail a required scheduler service through the same owner as PLC execution.
    pub(crate) fn scheduler_fault(&mut self, error: RuntimeError) -> RuntimeError {
        if self.faulted() {
            return error;
        }
        self.apply_fault(error, FaultDecision::from_fault_policy(self.fault_policy))
    }

    pub(super) fn apply_fault(
        &mut self,
        error: RuntimeError,
        decision: FaultDecision,
    ) -> RuntimeError {
        let debug = self.debug_control();
        if let Some(debug) = &debug {
            debug.clear_runtime_mutations();
        }
        self.state.latch_fault(error.clone());
        let mut reported = error.clone();
        if decision.apply_safe_state
            && let Err(safe_error) = self.apply_io_safe_state()
        {
            reported = RuntimeError::SafeStateFailed {
                root: error.to_string().into(),
                error: safe_error.to_string().into(),
            };
            if let Some(debug) = &debug {
                debug.push_runtime_event(RuntimeEvent::SafeStateFailed {
                    root: error.to_string(),
                    error: safe_error.to_string(),
                    time: self.current_time(),
                });
            }
        }
        self.metrics.record_fault();
        if let Some(debug) = &debug
            && let Some(record) = self.state.fault_record()
        {
            debug.push_runtime_event(RuntimeEvent::fault_with_record(
                record,
                &error,
                self.current_time(),
            ));
        }
        reported
    }

    /// Report a physical watchdog expiration through the common fault owner.
    pub fn watchdog_timeout(&mut self) -> RuntimeError {
        self.watchdog_fault(RuntimeError::WatchdogTimeout)
    }
    /// Apply the watchdog action while preserving a logical allowance diagnosis.
    pub(crate) fn watchdog_fault(&mut self, error: RuntimeError) -> RuntimeError {
        self.apply_fault(error, self.watchdog.decision())
    }
    /// Report a host panic without discarding safe-output delivery evidence.
    pub fn resource_panic(&mut self, message: impl Into<smol_str::SmolStr>) -> RuntimeError {
        self.apply_fault(
            RuntimeError::ResourcePanic(message.into()),
            FaultDecision {
                action: FaultAction::Halt,
                apply_safe_state: true,
            },
        )
    }
    /// Report a simulation fault using the installed controller's fault policy.
    pub fn simulation_fault(&mut self, message: impl Into<smol_str::SmolStr>) -> RuntimeError {
        self.apply_fault(
            RuntimeError::SimulationFault(message.into()),
            FaultDecision::from_fault_policy(self.fault_policy),
        )
    }
    /// Preserve the existing tooling reset: values and readiness history are kept.
    pub fn clear_fault(&mut self) {
        self.state.clear_fault();
    }
    pub(crate) fn set_output_commit_deadline(&mut self, deadline: Option<Instant>) {
        self.output_commit_deadline = deadline;
        self.output_commit_pause_baseline = self.debug_watchdog_pause_elapsed();
    }
    #[cfg(test)]
    #[must_use]
    pub(crate) fn output_commit_deadline(&self) -> Option<Instant> {
        self.output_commit_deadline
    }
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
}

impl CycleHooks for HostedCycle<'_> {
    fn has_program_hooks(&self) -> bool {
        true
    }

    fn program_started(&mut self, _: &smol_str::SmolStr) {
        self.program_timer = self.metrics.start_timer();
    }

    fn program_finished(&mut self, name: &smol_str::SmolStr) {
        if let Some(start) = self.program_timer.take() {
            self.metrics
                .record_profile_call("program", name, start.elapsed());
        }
    }

    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        let time = access.logical_time();
        let now_ms = u64::try_from(time.as_millis()).unwrap_or(0);
        match phase {
            CyclePhase::CycleStart => {
                if let Some(debug) = self.debug.clone() {
                    debug.begin_cycle();
                    let metadata = self.metadata;
                    debug.apply_live_values_cycle_start(
                        access,
                        metadata,
                        |access, target, value| write_lvalue(access, metadata, target, value),
                    )?;
                    debug.push_runtime_event(RuntimeEvent::CycleStart {
                        cycle: self.cycle,
                        time,
                    });
                }
            }
            CyclePhase::BeforeInputSample => {
                self.io.sample_inputs_with_cycle(access)?;
                if let Some(debug) = self.debug.clone() {
                    debug.apply_live_values_before_input_sample(access, self.metadata)?;
                }
            }
            CyclePhase::AfterInputSample => {
                self.ads.apply_inputs_with_cycle(access, now_ms)?;
                self.opcua.apply_inputs_with_cycle(access, now_ms)?;
                self.io.update_health();
            }
            CyclePhase::TaskOverrun { index, missed } => {
                let task = self
                    .tasks
                    .get(index)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                self.metrics.record_overrun(&task.name, missed);
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TaskOverrun {
                        name: task.name.clone(),
                        missed,
                        time,
                    });
                }
            }
            CyclePhase::TaskStart(index) => {
                let task = self
                    .tasks
                    .get(index)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                self.task_timer = self.metrics.start_timer();
                if let Some(debug) = &self.debug {
                    debug.set_current_thread(self.metadata.task_thread_id(&task.name));
                    debug.push_runtime_event(RuntimeEvent::TaskStart {
                        name: task.name.clone(),
                        priority: task.priority,
                        time,
                    });
                }
            }
            CyclePhase::TaskEnd(index) => {
                let task = self
                    .tasks
                    .get(index)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                if let Some(start) = self.task_timer.take() {
                    self.metrics.record_task(&task.name, start.elapsed());
                }
                if let Some(debug) = &self.debug {
                    debug.push_runtime_event(RuntimeEvent::TaskEnd {
                        name: task.name.clone(),
                        priority: task.priority,
                        time,
                    });
                }
            }
            CyclePhase::BackgroundStart(_) => {
                if let Some(debug) = &self.debug {
                    debug.set_current_thread(self.metadata.background_thread_id());
                }
            }
            CyclePhase::BackgroundEnd(_) => {}
            CyclePhase::BeforeOutputSample => {
                if self.retain.has_store() {
                    self.retain.mark_dirty();
                    if self.retain.should_save(time) {
                        let snapshot = RetainSnapshot {
                            inner: access.retain_snapshot()?,
                        };
                        self.retain.save_snapshot(snapshot, time)?;
                    }
                }
            }
            CyclePhase::AfterOutputSample => {
                if let Some(debug) = self.debug.clone() {
                    debug.apply_live_values_after_output_sample(access, self.metadata)?;
                }
                self.ads.capture_outputs_with_cycle(access, now_ms)?;
                self.opcua.capture_outputs_with_cycle(access, now_ms)?;
                #[cfg(feature = "debug")]
                if let Some(debug) = &self.debug {
                    let mut snapshot = self.io.snapshot_with_cycle(access)?;
                    snapshot.scan = Some(self.cycle);
                    snapshot.forced = debug
                        .forced_snapshot()
                        .io
                        .into_iter()
                        .map(|(address, _)| address)
                        .collect();
                    debug.push_io_snapshot(snapshot);
                }
                self.check_output_deadline()?;
            }
            CyclePhase::CycleEnd => {}
        }
        Ok(())
    }

    fn publish_outputs(&mut self, access: &CycleAccess<'_, '_>) -> Result<(), RuntimeError> {
        let result = self.io.publish_outputs_with_cycle(access);
        self.io.update_health();
        result
    }
}

impl HostedCycle<'_> {
    fn check_output_deadline(&self) -> Result<(), RuntimeError> {
        let pause = self
            .debug
            .as_ref()
            .map(DebugControl::watchdog_pause_elapsed)
            .unwrap_or_default();
        if self.output_deadline.is_some_and(|deadline| {
            let deadline = deadline
                .checked_add(pause.saturating_sub(self.output_pause_baseline))
                .unwrap_or(deadline);
            Instant::now() >= deadline
        }) {
            Err(RuntimeError::WatchdogTimeout)
        } else {
            Ok(())
        }
    }
}

fn write_lvalue(
    access: &mut CycleAccess<'_, '_>,
    metadata: &RuntimeMetadata,
    target: &LValue,
    value: Value,
) -> Result<(), RuntimeError> {
    // Partial selections are represented as a checked update of their declared
    // parent value; all other resolved targets become a common ValueRef.
    let (target, partial) = match target {
        LValue::Field { target, field } if crate::value::parse_partial_access(field).is_some() => {
            (&**target, crate::value::parse_partial_access(field))
        }
        other => (other, None),
    };
    let reference = crate::helper_eval::eval_storage_expr_with_abi(
        &access.store(),
        metadata.registry(),
        metadata.storage_abi(),
        &metadata.profile(),
        None,
        Some(metadata.stdlib()),
        &Expr::Ref(target.clone()),
    )?;
    let Value::Reference(Some(reference)) = reference else {
        return Err(RuntimeError::NullReference);
    };
    let value = match partial {
        Some(partial) => {
            crate::value::write_partial_access(access.read_reference(&reference)?, partial, value)
                .map_err(RuntimeError::from)?
        }
        None => value,
    };
    access.write_reference(reference.as_view(), value)
}
