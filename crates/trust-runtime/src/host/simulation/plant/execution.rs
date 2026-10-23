//! Fixed-step plant execution and transactional substep rollback.

use super::*;

impl PlantInstance {
    pub(super) fn new(config: PlantConfig, trace_capacity: usize) -> Self {
        let state = config.initial_state.clone();
        let actuators = config
            .actuators
            .iter()
            .cloned()
            .map(ActuatorState::new)
            .collect::<Vec<_>>();
        let uncertainties = config
            .uncertainties
            .iter()
            .cloned()
            .map(|effect| UncertaintyState::new(effect, config.master_seed, &config.id))
            .collect::<Vec<_>>();
        let mut uncertainties = uncertainties;
        // `initial_input` is already expressed in model engineering units; scaling
        // applies only when a PLC command crosses the process-image boundary.
        let initial_inputs = config.initial_input.clone();
        let mut initial_ideal = vec![0.0; config.feedback_targets.len()];
        model_output_into(&config.model, &state, &initial_inputs, &mut initial_ideal);
        let mut initial_measurement = initial_ideal.clone();
        for (channel, measurement) in initial_measurement.iter_mut().enumerate() {
            for effect in &mut uncertainties {
                effect.initialize_process_output(channel, *measurement);
            }
            for effect in &mut uncertainties {
                *measurement = effect.initialize_measurement(channel, *measurement);
            }
        }
        let measured_feedback = initial_measurement.clone();
        let initial_feedback = initial_measurement
            .iter()
            .enumerate()
            .map(|(channel, measurement)| {
                config.output_scale[channel] * measurement + config.output_offset[channel]
            })
            .collect::<Vec<_>>();
        let applied_input = actuators
            .iter()
            .map(ActuatorState::initial_applied)
            .collect::<Vec<_>>();
        let output_count = config.feedback_targets.len();
        let trace = TraceBuffer::new(
            trace_capacity,
            &config.id,
            config.model.kind(),
            config.command_sources.len(),
            state.len(),
            output_count,
            config.events.len(),
        );
        let event_capacity = config.events.len();
        let effective_model = config.model.clone();
        let model_workspace = ModelWorkspace::new(state.len());
        let command_count = initial_inputs.len();
        let parameter_overrides = vec![None; event_capacity];
        let input_history = InputHistory::for_config(&config);
        let sensor_bias = vec![0.0; output_count];
        let published_feedback = vec![0.0; output_count];
        let checkpoint = PlantCheckpoint {
            state: state.clone(),
            queued_feedback: initial_feedback.clone(),
            feedback_pending: true,
            model_time_nanos: 0,
            input_history: InputHistory::for_config(&config),
            actuators: actuators.clone(),
            uncertainties: uncertainties.clone(),
            event_cursor: 0,
            parameter_overrides: parameter_overrides.clone(),
            sensor_bias: sensor_bias.clone(),
            held_command: initial_inputs.clone(),
            latest_command: initial_inputs.clone(),
            latest_applied_input: applied_input.clone(),
            latest_ideal_output: initial_ideal.clone(),
            latest_measured_feedback: measured_feedback.clone(),
        };
        Self {
            config,
            state,
            accumulator_nanos: 0,
            queued_feedback: initial_feedback,
            feedback_pending: true,
            model_time_nanos: 0,
            input_history,
            actuators,
            uncertainties,
            event_cursor: 0,
            parameter_overrides,
            sensor_bias,
            last_substeps: 0,
            held_command: initial_inputs.clone(),
            latest_command: initial_inputs,
            latest_applied_input: applied_input,
            latest_ideal_output: initial_ideal,
            latest_measured_feedback: measured_feedback,
            published_feedback,
            fired_events: Vec::with_capacity(event_capacity),
            last_fault: None,
            trace,
            effective_model,
            effective_commands: vec![0.0; command_count],
            delayed_inputs: vec![0.0; command_count],
            model_workspace,
            checkpoint,
        }
    }

    pub(super) fn advance(
        &mut self,
        elapsed_nanos: i64,
        max_substeps: usize,
        next_trace_sequence: &mut u64,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        if let Err(error) =
            self.advance_inner(elapsed_nanos, max_substeps, next_trace_sequence, runtime)
        {
            let fault = RuntimeError::SimulationFault(
                format!(
                    "plant '{}' model '{}' at {} ns substep {} plant advance failed: {error}",
                    self.config.id,
                    self.config.model.kind(),
                    self.model_time_nanos,
                    self.last_substeps
                )
                .into(),
            );
            self.last_fault = Some(fault.to_string());
            return Err(fault);
        }
        self.last_fault = None;
        Ok(())
    }

    fn advance_inner(
        &mut self,
        elapsed_nanos: i64,
        max_substeps: usize,
        next_trace_sequence: &mut u64,
        runtime: &Runtime,
    ) -> Result<(), RuntimeError> {
        self.accumulator_nanos = self.accumulator_nanos.saturating_add(elapsed_nanos.max(0));
        let step_nanos = self.config.step.as_nanos();
        let required = (self.accumulator_nanos / step_nanos) as usize;
        if required > max_substeps {
            return Err(RuntimeError::SimulationFault(
                format!("plant '{}' exceeded substep limit", self.config.id).into(),
            ));
        }
        self.last_substeps = required;
        for (channel, source) in self.config.command_sources.iter().enumerate() {
            let plc_command = source.read(runtime)?;
            self.held_command[channel] =
                self.config.input_scale[channel] * plc_command + self.config.input_offset[channel];
        }
        if required == 0 {
            return Ok(());
        }
        let h = step_nanos as f64 / 1_000_000_000.0;
        for substep in 0..required {
            self.save_checkpoint();
            let result = (|| {
                self.effective_commands.clone_from(&self.held_command);
                self.latest_command.clone_from(&self.held_command);
                self.fired_events.clear();
                self.apply_due_events(self.model_time_nanos + step_nanos);
                let time_s = (self.model_time_nanos + step_nanos) as f64 / 1_000_000_000.0;
                for (channel, effective_command) in self.effective_commands.iter_mut().enumerate() {
                    for effect in &mut self.uncertainties {
                        *effective_command =
                            effect.apply_command(channel, *effective_command, time_s, h)?;
                    }
                    self.latest_command[channel] = *effective_command;
                    self.latest_applied_input[channel] =
                        self.actuators[channel].advance(*effective_command, h);
                    for effect in &mut self.uncertainties {
                        self.latest_applied_input[channel] = effect.apply_process_input(
                            channel,
                            self.latest_applied_input[channel],
                            time_s,
                            h,
                        )?;
                    }
                }
                self.config
                    .validity
                    .check_input(&self.config.id, &self.latest_applied_input)?;
                // Transport history is in model units, after command scaling,
                // actuator processing and process-input uncertainty for each channel.
                self.input_history
                    .record(self.model_time_nanos, &self.latest_applied_input);
                self.prune_input_history();
                self.effective_model.reset_values_from(&self.config.model);
                apply_parameter_overrides(
                    &mut self.effective_model,
                    &self.config.events,
                    &self.parameter_overrides,
                );
                apply_parameter_uncertainties(
                    &mut self.effective_model,
                    &mut self.uncertainties,
                    time_s,
                )?;
                match &self.effective_model {
                    PlantModelConfig::StaticGain { .. }
                    | PlantModelConfig::PiecewiseLinear { .. } => {}
                    PlantModelConfig::FirstOrder {
                        gain,
                        time_constant_s,
                        input_bias,
                        output_bias,
                    } => {
                        let target =
                            output_bias + gain * (self.latest_applied_input[0] - input_bias);
                        let a = (-h / time_constant_s).exp();
                        self.state[0] = target + (self.state[0] - target) * a;
                    }
                    PlantModelConfig::FirstOrderDeadTime {
                        gain,
                        time_constant_s,
                        input_bias,
                        output_bias,
                        dead_time_s,
                        initial_delayed_input,
                    } => advance_fopdt(
                        &mut self.state[0],
                        self.model_time_nanos,
                        step_nanos,
                        self.input_history.channel(0),
                        *gain,
                        *time_constant_s,
                        *input_bias,
                        *output_bias,
                        *dead_time_s,
                        *initial_delayed_input,
                    ),
                    PlantModelConfig::Integrator {
                        gain, input_bias, ..
                    } => {
                        self.state[0] += h * gain * (self.latest_applied_input[0] - input_bias);
                    }
                    PlantModelConfig::IntegratorDeadTime {
                        gain,
                        input_bias,
                        dead_time_s,
                        initial_delayed_input,
                        ..
                    } => advance_delayed_integrator(
                        &mut self.state[0],
                        self.model_time_nanos,
                        step_nanos,
                        self.input_history.channel(0),
                        *gain,
                        *input_bias,
                        *dead_time_s,
                        *initial_delayed_input,
                    ),
                    PlantModelConfig::GravityTank {
                        area_m2,
                        outflow_coefficient,
                    } => rk4_gravity_tank(
                        &mut self.state[0],
                        self.latest_applied_input[0],
                        h,
                        *area_m2,
                        *outflow_coefficient,
                    ),
                    PlantModelConfig::GravityTankCascade {
                        area_m2,
                        outflow_coefficient,
                    } => rk4_gravity_tank_cascade(
                        &mut self.state,
                        self.latest_applied_input[0],
                        h,
                        *area_m2,
                        *outflow_coefficient,
                    ),
                    PlantModelConfig::RobotJoint {
                        inertia,
                        gravity_torque,
                        viscous_friction,
                        coulomb_friction,
                        friction_smoothing,
                    } => rk4_robot_joint(
                        &mut self.state,
                        self.latest_applied_input[0],
                        h,
                        *inertia,
                        *gravity_torque,
                        *viscous_friction,
                        *coulomb_friction,
                        *friction_smoothing,
                    ),
                    PlantModelConfig::SecondOrder {
                        gain,
                        natural_frequency,
                        damping_ratio,
                        input_bias,
                        ..
                    } => rk4_second_order(
                        &mut self.state,
                        self.latest_applied_input[0],
                        h,
                        *gain,
                        *natural_frequency,
                        *damping_ratio,
                        *input_bias,
                    ),
                    PlantModelConfig::StateSpace { a, b, .. } => {
                        if self.config.discrete_state_space {
                            model::step_discrete_state_space(
                                &mut self.state,
                                &self.latest_applied_input,
                                a,
                                b,
                                &mut self.model_workspace,
                            );
                        } else if self.input_history.enabled() {
                            state_space_delay::advance(
                                &mut self.state,
                                self.model_time_nanos..self.model_time_nanos + step_nanos,
                                &self.input_history,
                                &self.config.initial_input,
                                (a, b),
                                &mut self.model_workspace,
                                &mut self.delayed_inputs,
                            );
                        } else {
                            rk4_state_space(
                                &mut self.state,
                                &self.latest_applied_input,
                                h,
                                a,
                                b,
                                &mut self.model_workspace,
                            );
                        }
                    }
                }
                for (channel, state) in self.state.iter_mut().enumerate() {
                    for effect in &mut self.uncertainties {
                        *state = effect.apply_state(channel, *state, time_s, h)?;
                    }
                }
                self.config
                    .validity
                    .check_state(&self.config.id, &self.state)?;
                model_output_into(
                    &self.effective_model,
                    &self.state,
                    if matches!(self.effective_model, PlantModelConfig::StateSpace { .. })
                        && self.input_history.enabled()
                    {
                        &self.delayed_inputs
                    } else {
                        &self.latest_applied_input
                    },
                    &mut self.latest_ideal_output,
                );
                self.latest_measured_feedback
                    .clone_from(&self.latest_ideal_output);
                for (channel, output) in self.latest_measured_feedback.iter_mut().enumerate() {
                    for effect in &mut self.uncertainties {
                        *output = effect.apply_process_output(channel, *output, time_s, h)?;
                    }
                }
                self.config
                    .validity
                    .check_output(&self.config.id, &self.latest_measured_feedback)?;
                for (channel, output) in self.latest_measured_feedback.iter_mut().enumerate() {
                    *output += self.sensor_bias[channel];
                    for effect in &mut self.uncertainties {
                        *output = effect.apply_measurement(channel, *output, time_s, h)?;
                    }
                    if !output.is_finite() {
                        return Err(RuntimeError::SimulationFault(
                            format!(
                                "plant '{}' produced a nonfinite measurement on channel {channel}",
                                self.config.id
                            )
                            .into(),
                        ));
                    }
                }
                for (channel, queued) in self.queued_feedback.iter_mut().enumerate() {
                    *queued = self.config.output_scale[channel]
                        * self.latest_measured_feedback[channel]
                        + self.config.output_offset[channel];
                }
                if self.queued_feedback.iter().any(|value| !value.is_finite()) {
                    return Err(RuntimeError::SimulationFault(
                        format!(
                            "plant '{}' produced nonfinite scaled feedback",
                            self.config.id
                        )
                        .into(),
                    ));
                }
                self.feedback_pending = true;
                self.model_time_nanos += step_nanos;
                self.trace.record(
                    *next_trace_sequence,
                    Duration::from_nanos(self.model_time_nanos),
                    &self.state,
                    &self.latest_command,
                    &self.latest_applied_input,
                    &self.latest_ideal_output,
                    &self.latest_measured_feedback,
                    &self.queued_feedback,
                    &self.fired_events,
                );
                *next_trace_sequence = next_trace_sequence.saturating_add(1);
                Ok(())
            })();
            if let Err(error) = result {
                self.restore_checkpoint();
                self.last_substeps = substep + 1;
                return Err(error);
            }
            self.accumulator_nanos -= step_nanos;
        }
        self.prune_input_history();
        Ok(())
    }

    fn save_checkpoint(&mut self) {
        self.checkpoint.state.clone_from(&self.state);
        self.checkpoint
            .queued_feedback
            .clone_from(&self.queued_feedback);
        self.checkpoint.feedback_pending = self.feedback_pending;
        self.checkpoint.model_time_nanos = self.model_time_nanos;
        self.checkpoint
            .input_history
            .copy_runtime_from(&self.input_history);
        self.checkpoint.actuators.clone_from(&self.actuators);
        for (target, source) in self
            .checkpoint
            .uncertainties
            .iter_mut()
            .zip(&self.uncertainties)
        {
            target.copy_runtime_from(source);
        }
        self.checkpoint.event_cursor = self.event_cursor;
        self.checkpoint
            .parameter_overrides
            .clone_from(&self.parameter_overrides);
        self.checkpoint.sensor_bias.clone_from(&self.sensor_bias);
        self.checkpoint.held_command.clone_from(&self.held_command);
        self.checkpoint
            .latest_command
            .clone_from(&self.latest_command);
        self.checkpoint
            .latest_applied_input
            .clone_from(&self.latest_applied_input);
        self.checkpoint
            .latest_ideal_output
            .clone_from(&self.latest_ideal_output);
        self.checkpoint
            .latest_measured_feedback
            .clone_from(&self.latest_measured_feedback);
    }

    fn restore_checkpoint(&mut self) {
        self.state.clone_from(&self.checkpoint.state);
        self.queued_feedback
            .clone_from(&self.checkpoint.queued_feedback);
        self.feedback_pending = self.checkpoint.feedback_pending;
        self.model_time_nanos = self.checkpoint.model_time_nanos;
        self.input_history
            .copy_runtime_from(&self.checkpoint.input_history);
        self.actuators.clone_from(&self.checkpoint.actuators);
        for (target, source) in self
            .uncertainties
            .iter_mut()
            .zip(&self.checkpoint.uncertainties)
        {
            target.copy_runtime_from(source);
        }
        self.event_cursor = self.checkpoint.event_cursor;
        self.parameter_overrides
            .clone_from(&self.checkpoint.parameter_overrides);
        self.sensor_bias.clone_from(&self.checkpoint.sensor_bias);
        self.held_command.clone_from(&self.checkpoint.held_command);
        self.latest_command
            .clone_from(&self.checkpoint.latest_command);
        self.latest_applied_input
            .clone_from(&self.checkpoint.latest_applied_input);
        self.latest_ideal_output
            .clone_from(&self.checkpoint.latest_ideal_output);
        self.latest_measured_feedback
            .clone_from(&self.checkpoint.latest_measured_feedback);
    }

    fn prune_input_history(&mut self) {
        self.input_history
            .prune(self.model_time_nanos, self.config.step.as_nanos());
    }

    fn apply_due_events(&mut self, step_boundary_nanos: i64) {
        while let Some(event) = self.config.events.get(self.event_cursor) {
            if event.at_nanos > step_boundary_nanos {
                break;
            }
            let event_index = self.event_cursor;
            let action = &event.action;
            self.fired_events.push(action.kind());
            let reset = matches!(action, PlantEventAction::ResetPlant);
            match action {
                PlantEventAction::SetState { channel, value } => self.state[*channel] = *value,
                PlantEventAction::AddState { channel, value } => self.state[*channel] += *value,
                PlantEventAction::SetParameter { value, .. } => {
                    self.parameter_overrides[event_index] = Some(*value);
                }
                PlantEventAction::SetSensorBias { channel, value } => {
                    self.sensor_bias[*channel] = *value;
                }
                PlantEventAction::StickActuator => self.actuators[0].stick(),
                PlantEventAction::ReleaseActuator => self.actuators[0].release(),
                PlantEventAction::ResetPlant => {}
            }
            self.event_cursor += 1;
            if reset {
                self.reset_plant_state();
            }
        }
    }

    fn reset_plant_state(&mut self) {
        self.state.clone_from(&self.config.initial_state);
        for (state, config) in self.actuators.iter_mut().zip(&self.config.actuators) {
            *state = ActuatorState::new(config.clone());
        }
        for (state, config) in self
            .uncertainties
            .iter_mut()
            .zip(&self.config.uncertainties)
        {
            *state =
                UncertaintyState::new(config.clone(), self.config.master_seed, &self.config.id);
        }
        self.parameter_overrides.fill(None);
        self.sensor_bias.fill(0.0);
        self.input_history.clear();
    }
}
