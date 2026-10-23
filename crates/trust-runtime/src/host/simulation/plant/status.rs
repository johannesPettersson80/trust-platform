//! Preallocated bounded trace storage for completed plant substeps.

use crate::value::Duration;
use crate::{error::RuntimeError, Runtime};

use super::super::SimulationTraceRow;

#[derive(Debug, Clone)]
pub(super) struct TraceBuffer {
    slots: Vec<SimulationTraceRow>,
    next: usize,
    len: usize,
    dropped: u64,
}

impl TraceBuffer {
    pub(super) fn new(
        capacity: usize,
        plant_id: &str,
        model: &str,
        command_count: usize,
        state_count: usize,
        output_count: usize,
        event_capacity: usize,
    ) -> Self {
        let template = SimulationTraceRow {
            sequence: 0,
            plant_id: plant_id.to_owned(),
            model: model.to_owned(),
            simulation_time: Duration::ZERO,
            state: vec![0.0; state_count],
            transformed_command: vec![0.0; command_count],
            applied_input: vec![0.0; command_count],
            ideal_output: vec![0.0; output_count],
            measured_feedback: vec![0.0; output_count],
            queued_feedback: vec![0.0; output_count],
            fired_events: Vec::with_capacity(event_capacity),
        };
        Self {
            slots: vec![template; capacity],
            next: 0,
            len: 0,
            dropped: 0,
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn record(
        &mut self,
        sequence: u64,
        simulation_time: Duration,
        state: &[f64],
        transformed_command: &[f64],
        applied_input: &[f64],
        ideal_output: &[f64],
        measured_feedback: &[f64],
        queued_feedback: &[f64],
        fired_events: &[&'static str],
    ) {
        if self.slots.is_empty() {
            return;
        }
        let slot = &mut self.slots[self.next];
        slot.sequence = sequence;
        slot.simulation_time = simulation_time;
        slot.state.clone_from_slice(state);
        slot.transformed_command
            .clone_from_slice(transformed_command);
        slot.applied_input.clone_from_slice(applied_input);
        slot.ideal_output.clone_from_slice(ideal_output);
        slot.measured_feedback.clone_from_slice(measured_feedback);
        slot.queued_feedback.clone_from_slice(queued_feedback);
        slot.fired_events.clear();
        slot.fired_events.extend_from_slice(fired_events);
        self.next = (self.next + 1) % self.slots.len();
        if self.len < self.slots.len() {
            self.len += 1;
        } else {
            self.dropped = self.dropped.saturating_add(1);
        }
    }

    pub(super) fn rows(&self) -> Vec<SimulationTraceRow> {
        if self.len == 0 {
            return Vec::new();
        }
        let first = if self.len == self.slots.len() {
            self.next
        } else {
            0
        };
        (0..self.len)
            .map(|offset| self.slots[(first + offset) % self.slots.len()].clone())
            .collect()
    }

    pub(super) fn dropped(&self) -> u64 {
        self.dropped
    }

    pub(super) fn len(&self) -> usize {
        self.len
    }

    pub(super) fn clear(&mut self) {
        self.next = 0;
        self.len = 0;
        self.dropped = 0;
    }
}

impl super::PlantSystem {
    pub(in super::super) fn statuses(&self) -> Vec<super::super::PlantStatus> {
        self.plants
            .iter()
            .map(super::PlantInstance::status)
            .collect()
    }

    pub(in super::super) fn trace_rows(&self) -> Vec<SimulationTraceRow> {
        let mut rows = self
            .plants
            .iter()
            .flat_map(|plant| plant.trace.rows())
            .collect::<Vec<_>>();
        rows.sort_by_key(|row| row.sequence);
        rows
    }

    pub(in super::super) fn trace_stats(&self) -> Vec<super::super::PlantTraceStats> {
        self.plants
            .iter()
            .map(|plant| super::super::PlantTraceStats {
                plant_id: plant.config.id.clone(),
                retained_rows: plant.trace.len(),
                dropped_rows: plant.trace.dropped(),
            })
            .collect()
    }

    pub(in super::super) fn dropped_trace_rows(&self) -> u64 {
        self.plants.iter().map(|plant| plant.trace.dropped()).sum()
    }

    pub(in super::super) fn clear_trace(&mut self) {
        for plant in &mut self.plants {
            plant.trace.clear();
        }
    }

    pub(in super::super) fn clear_plant_trace(
        &mut self,
        plant_id: &str,
    ) -> Option<super::super::PlantTraceClearResult> {
        let plant = self
            .plants
            .iter_mut()
            .find(|plant| plant.config.id == plant_id)?;
        let result = super::super::PlantTraceClearResult {
            plant_id: plant_id.to_string(),
            prior_row_count: plant.trace.len(),
            prior_dropped_row_count: plant.trace.dropped(),
        };
        plant.trace.clear();
        Some(result)
    }
}

impl super::PlantInstance {
    pub(super) fn publish(&mut self, runtime: &mut Runtime) -> Result<(), RuntimeError> {
        if !self.feedback_pending {
            return Ok(());
        }
        let values = &self.queued_feedback;
        // Program variables are written after the process-image batch, and only when every value converts.
        let stored_result = self
            .config
            .feedback_targets
            .iter()
            .zip(values)
            .filter_map(|(target, value)| match target {
                super::signal::PlantSignal::Symbol(symbol) => Some((symbol, *value)),
                super::signal::PlantSignal::Address(_)
                | super::signal::PlantSignal::Controller(_) => None,
            })
            .map(|(symbol, value)| {
                let (reference, current) = super::signal::resolve(runtime, symbol)?;
                let encoded =
                    super::numeric_io::encode_storage(&current, value).map_err(|error| {
                        RuntimeError::SimulationFault(
                            format!("plant signal {symbol}: {error}").into(),
                        )
                    })?;
                Ok((reference, encoded))
            })
            .collect::<Result<Vec<_>, RuntimeError>>();
        let stored = match stored_result {
            Ok(stored) => stored,
            Err(error) => {
                let fault = self.feedback_fault(error);
                self.last_fault = Some(fault.to_string());
                return Err(fault);
            }
        };
        // Control.PID measurements (`pid:`) are given after every other target is written.
        let controllers_result = self
            .config
            .feedback_targets
            .iter()
            .zip(values)
            .filter_map(|(target, value)| match target {
                super::signal::PlantSignal::Controller(path) => Some((path, *value)),
                _ => None,
            })
            .map(|(path, value)| {
                super::signal::controller_instance(runtime, path).map(|instance| (instance, value))
            })
            .collect::<Result<Vec<_>, RuntimeError>>();
        let controllers = match controllers_result {
            Ok(controllers) => controllers,
            Err(error) => {
                let fault = self.feedback_fault(error);
                self.last_fault = Some(fault.to_string());
                return Err(fault);
            }
        };
        let pending_result = self
            .config
            .feedback_targets
            .iter()
            .zip(values)
            .filter_map(|(target, value)| target.address().map(|address| (address, value)))
            .map(|(address, value)| {
                super::numeric_io::encode_numeric(&runtime.io(), address, *value)
                    .map(|encoded| (address.clone(), encoded))
            })
            .chain(
                self.config
                    .actuator_feedback_targets
                    .iter()
                    .enumerate()
                    .filter_map(|(channel, address)| {
                        address.as_ref().map(|address| (channel, address))
                    })
                    .map(|(channel, address)| {
                        let applied = (self.actuators[channel].current_applied()
                            - self.config.input_offset[channel])
                            / self.config.input_scale[channel];
                        super::numeric_io::encode_numeric(&runtime.io(), address, applied)
                            .map(|encoded| (address.clone(), encoded))
                    }),
            )
            .collect::<Result<Vec<_>, _>>()
            .and_then(|pending| {
                runtime.write_io_batch(&pending)?;
                Ok(pending)
            });
        let pending = match pending_result {
            Ok(pending) => pending,
            Err(error) => {
                let fault = self.feedback_fault(error);
                self.last_fault = Some(fault.to_string());
                return Err(fault);
            }
        };
        debug_assert_eq!(
            pending.len() + stored.len() + controllers.len(),
            self.config.feedback_targets.len()
                + self
                    .config
                    .actuator_feedback_targets
                    .iter()
                    .flatten()
                    .count()
        );
        for (reference, value) in stored {
            if runtime.write_reference(&reference, value).is_err() {
                let fault = self.feedback_fault(RuntimeError::SimulationFault(
                    "a symbolic plant feedback target could not be written".into(),
                ));
                self.last_fault = Some(fault.to_string());
                return Err(fault);
            }
        }
        for (instance, value) in controllers {
            runtime.set_controller_measurement(instance, value);
        }
        self.published_feedback.clone_from(values);
        self.feedback_pending = false;
        Ok(())
    }

    fn feedback_fault(&self, error: RuntimeError) -> RuntimeError {
        RuntimeError::SimulationFault(
            format!(
                "plant '{}' model '{}' at {} ns substep {} feedback publish failed: {error}",
                self.config.id,
                self.config.model.kind(),
                self.model_time_nanos,
                self.last_substeps
            )
            .into(),
        )
    }

    fn status(&self) -> super::super::PlantStatus {
        let time_s = self.model_time_nanos as f64 / 1_000_000_000.0;
        super::super::PlantStatus {
            id: self.config.id.clone(),
            model: self.config.model.kind().to_owned(),
            enabled: true,
            step: self.config.step,
            simulated_time: Duration::from_nanos(self.model_time_nanos),
            state: self.state.clone(),
            transformed_command: self.latest_command.clone(),
            applied_input: self.latest_applied_input.clone(),
            ideal_output: self.latest_ideal_output.clone(),
            measured_feedback: self.latest_measured_feedback.clone(),
            published_feedback: self.published_feedback.clone(),
            actuators: self
                .actuators
                .iter()
                .map(|actuator| actuator.status())
                .collect(),
            active_uncertainties: self
                .uncertainties
                .iter()
                .filter(|effect| effect.is_active(time_s))
                .map(|effect| effect.id().to_owned())
                .collect(),
            active_dropouts: self
                .uncertainties
                .iter()
                .filter(|effect| effect.dropout_active(time_s))
                .map(|effect| effect.id().to_owned())
                .collect(),
            pending_events: self.config.events.len().saturating_sub(self.event_cursor),
            delay_buffer_depth: self.input_history.len(),
            accumulated_remainder: Duration::from_nanos(self.accumulator_nanos),
            last_substeps: self.last_substeps,
            last_fault: self.last_fault.clone(),
        }
    }
}
