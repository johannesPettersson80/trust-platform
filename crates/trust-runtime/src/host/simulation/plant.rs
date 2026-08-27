//! Dynamic process models for simulation-mode process-image feedback.

use std::collections::VecDeque;

use serde::Deserialize;

use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::value::Duration;
use crate::Runtime;

#[path = "plant/effects.rs"]
mod effects;
#[path = "plant/events.rs"]
mod events;
#[path = "plant/execution.rs"]
mod execution;
#[path = "plant/model.rs"]
mod model;
#[path = "plant/numeric_io.rs"]
mod numeric_io;
#[cfg(test)]
#[path = "plant/resource_tests.rs"]
mod resource_tests;
#[path = "plant/status.rs"]
mod status;
#[path = "plant/uncertainty.rs"]
mod uncertainty;
#[path = "plant/validation.rs"]
mod validation;

use effects::{ActuatorConfig, ActuatorSection, ActuatorState};
pub(super) use events::PlantEventSection;
use events::{PlantEventAction, PlantEventConfig};
use model::{
    advance_fopdt, model_output_into, rk4_second_order, rk4_state_space, ModelWorkspace,
    PlantModelConfig,
};
use numeric_io::read_numeric;
use status::TraceBuffer;
use uncertainty::{UncertaintyConfig, UncertaintySection, UncertaintyState};
use validation::{
    apply_parameter_overrides, apply_parameter_uncertainties, channel_setting, checked_step,
    finite, invalid, model_supports_parameter, parse_addresses, valid_id,
};

const DEFAULT_STEP_MS: i64 = 10;
const DEFAULT_MAX_SUBSTEPS: usize = 10_000;

pub(crate) const SUPPORTED_MODEL_KINDS: &[&str] = &[
    "static_gain",
    "first_order",
    "first_order_dead_time",
    "integrator",
    "piecewise_linear",
    "second_order",
    "state_space",
];

#[derive(Debug, Clone)]
pub struct PlantSystemConfig {
    pub default_step: Duration,
    pub max_substeps_per_cycle: usize,
    pub plants: Vec<PlantConfig>,
    trace_capacity: usize,
}

impl Default for PlantSystemConfig {
    fn default() -> Self {
        Self {
            default_step: Duration::from_millis(DEFAULT_STEP_MS),
            max_substeps_per_cycle: DEFAULT_MAX_SUBSTEPS,
            plants: Vec::new(),
            trace_capacity: 0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct PlantConfig {
    id: String,
    model: PlantModelConfig,
    command_sources: Vec<IoAddress>,
    feedback_targets: Vec<IoAddress>,
    step: Duration,
    initial_state: Vec<f64>,
    initial_input: Vec<f64>,
    input_scale: Vec<f64>,
    input_offset: Vec<f64>,
    output_scale: Vec<f64>,
    output_offset: Vec<f64>,
    actuators: Vec<ActuatorConfig>,
    uncertainties: Vec<UncertaintyConfig>,
    master_seed: u64,
    enabled: bool,
    events: Vec<PlantEventConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct PlantSection {
    id: String,
    name: Option<String>,
    description: Option<String>,
    model: String,
    command_sources: Vec<String>,
    feedback_targets: Vec<String>,
    step_ms: Option<u64>,
    initial_state: Option<Vec<f64>>,
    initial_input: Option<Vec<f64>>,
    input_scale: Option<Vec<f64>>,
    input_offset: Option<Vec<f64>>,
    output_scale: Option<Vec<f64>>,
    output_offset: Option<Vec<f64>>,
    parameters: PlantParametersSection,
    enabled: Option<bool>,
    actuator: Option<ActuatorSection>,
    #[serde(default)]
    uncertainties: Vec<UncertaintySection>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct PlantParametersSection {
    gain: Option<f64>,
    time_constant_s: Option<f64>,
    input_bias: Option<f64>,
    output_bias: Option<f64>,
    input_points: Option<Vec<f64>>,
    output_points: Option<Vec<f64>>,
    natural_frequency_rad_s: Option<f64>,
    damping_ratio: Option<f64>,
    a: Option<Vec<Vec<f64>>>,
    b: Option<Vec<Vec<f64>>>,
    c: Option<Vec<Vec<f64>>>,
    d: Option<Vec<Vec<f64>>>,
    dead_time_s: Option<f64>,
    initial_delayed_input: Option<f64>,
}

impl PlantParametersSection {
    fn validate_for_model(&self, model: &str) -> Result<(), RuntimeError> {
        let allowed: &[&str] = match model {
            "static_gain" | "integrator" => &["gain", "input_bias", "output_bias"],
            "first_order" => &["gain", "time_constant_s", "input_bias", "output_bias"],
            "first_order_dead_time" => &[
                "gain",
                "time_constant_s",
                "input_bias",
                "output_bias",
                "dead_time_s",
                "initial_delayed_input",
            ],
            "piecewise_linear" => &["input_points", "output_points"],
            "second_order" => &[
                "gain",
                "natural_frequency_rad_s",
                "damping_ratio",
                "input_bias",
                "output_bias",
            ],
            "state_space" => &["a", "b", "c", "d"],
            _ => return Ok(()),
        };
        let present = [
            ("gain", self.gain.is_some()),
            ("time_constant_s", self.time_constant_s.is_some()),
            ("input_bias", self.input_bias.is_some()),
            ("output_bias", self.output_bias.is_some()),
            ("input_points", self.input_points.is_some()),
            ("output_points", self.output_points.is_some()),
            (
                "natural_frequency_rad_s",
                self.natural_frequency_rad_s.is_some(),
            ),
            ("damping_ratio", self.damping_ratio.is_some()),
            ("a", self.a.is_some()),
            ("b", self.b.is_some()),
            ("c", self.c.is_some()),
            ("d", self.d.is_some()),
            ("dead_time_s", self.dead_time_s.is_some()),
            (
                "initial_delayed_input",
                self.initial_delayed_input.is_some(),
            ),
        ];
        if let Some((field, _)) = present
            .into_iter()
            .find(|(field, is_present)| *is_present && !allowed.contains(field))
        {
            return invalid(format!(
                "plants.parameters.{field} does not belong to model '{model}'"
            ));
        }
        Ok(())
    }
}

pub(super) fn build_config(
    sections: Vec<PlantSection>,
    event_sections: Vec<PlantEventSection>,
    default_step_ms: Option<u64>,
    max_substeps: Option<usize>,
    master_seed: u64,
    trace_capacity: usize,
) -> Result<PlantSystemConfig, RuntimeError> {
    let default_step = checked_step("simulation.plant_step_ms", default_step_ms.unwrap_or(10))?;
    let max_substeps_per_cycle = max_substeps.unwrap_or(DEFAULT_MAX_SUBSTEPS);
    if max_substeps_per_cycle == 0 {
        return invalid("simulation.max_plant_substeps_per_cycle must be > 0");
    }
    let mut plants = sections
        .into_iter()
        .map(|section| section.into_config(default_step, master_seed))
        .collect::<Result<Vec<_>, _>>()?;
    let mut ids = std::collections::HashSet::new();
    if plants.iter().any(|plant| !ids.insert(plant.id.clone())) {
        return invalid("plant ids must be unique");
    }
    for section in event_sections {
        let plant = plants
            .iter_mut()
            .find(|plant| plant.id == section.plant)
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!("plant event references unknown plant '{}'", section.plant).into(),
                )
            })?;
        if !plant.enabled {
            return invalid(format!(
                "plant event references disabled plant '{}'",
                plant.id
            ));
        }
        plant.events.push(section.into_config(
            plant.initial_state.len(),
            plant.feedback_targets.len(),
            plant.command_sources.len() == 1,
        )?);
        if let Some(PlantEventConfig {
            action: PlantEventAction::SetParameter { parameter, .. },
            ..
        }) = plant.events.last()
        {
            if !model_supports_parameter(&plant.model, parameter) {
                return invalid(format!(
                    "plant '{}' has no parameter '{parameter}'",
                    plant.id
                ));
            }
        }
    }
    for plant in &mut plants {
        plant.events.sort_by_key(|event| event.at_nanos);
    }
    plants.retain(|plant| plant.enabled);
    plants.sort_by(|left, right| left.id.as_bytes().cmp(right.id.as_bytes()));
    for (index, plant) in plants.iter().enumerate() {
        if plants[..index].iter().any(|other| {
            plant.feedback_targets.iter().any(|target| {
                other
                    .feedback_targets
                    .iter()
                    .any(|owned| super::addresses_overlap(owned, target))
            })
        }) {
            return invalid(format!(
                "plant '{}' feedback target is already owned by another plant",
                plant.id
            ));
        }
    }
    Ok(PlantSystemConfig {
        default_step,
        max_substeps_per_cycle,
        plants,
        trace_capacity,
    })
}

impl PlantSystemConfig {
    pub(super) fn trace_capacity(&self) -> usize {
        self.trace_capacity
    }

    pub(super) fn feedback_targets(&self) -> impl Iterator<Item = (&str, &IoAddress)> {
        self.plants.iter().flat_map(|plant| {
            plant
                .feedback_targets
                .iter()
                .map(move |target| (plant.id.as_str(), target))
        })
    }
}

impl PlantSection {
    fn into_config(
        mut self,
        default_step: Duration,
        master_seed: u64,
    ) -> Result<PlantConfig, RuntimeError> {
        if !valid_id(&self.id) {
            return invalid("plants.id must match [A-Za-z][A-Za-z0-9_-]*");
        }
        if self
            .name
            .as_ref()
            .is_some_and(|name| name.trim().is_empty())
        {
            return invalid("plants.name must contain non-whitespace text when provided");
        }
        // Presentation metadata is preserved by the configuration document and
        // plant service. It deliberately does not enter numerical state.
        let _presentation_metadata = (self.name.take(), self.description.take());
        if self.command_sources.is_empty() || self.feedback_targets.is_empty() {
            return invalid("plant command_sources and feedback_targets must not be empty");
        }
        self.parameters.validate_for_model(&self.model)?;
        let command_sources = parse_addresses(&self.command_sources, IoArea::Output, "command")?;
        let feedback_targets = parse_addresses(&self.feedback_targets, IoArea::Input, "feedback")?;
        let command_count = command_sources.len();
        let feedback_count = feedback_targets.len();
        let requested_state = self.initial_state;
        let model = match self.model.as_str() {
            "static_gain" => PlantModelConfig::StaticGain {
                gain: finite(
                    "plants.parameters.gain",
                    self.parameters.gain.unwrap_or(1.0),
                )?,
                input_bias: finite(
                    "plants.parameters.input_bias",
                    self.parameters.input_bias.unwrap_or(0.0),
                )?,
                output_bias: finite(
                    "plants.parameters.output_bias",
                    self.parameters.output_bias.unwrap_or(0.0),
                )?,
            },
            "first_order" => {
                let gain = finite(
                    "plants.parameters.gain",
                    self.parameters.gain.unwrap_or(1.0),
                )?;
                let time_constant_s = finite(
                    "plants.parameters.time_constant_s",
                    self.parameters.time_constant_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "first_order requires parameters.time_constant_s".into(),
                        )
                    })?,
                )?;
                if time_constant_s <= 0.0 {
                    return invalid("plants.parameters.time_constant_s must be > 0");
                }
                PlantModelConfig::FirstOrder {
                    gain,
                    time_constant_s,
                    input_bias: finite(
                        "plants.parameters.input_bias",
                        self.parameters.input_bias.unwrap_or(0.0),
                    )?,
                    output_bias: finite(
                        "plants.parameters.output_bias",
                        self.parameters.output_bias.unwrap_or(0.0),
                    )?,
                }
            }
            "first_order_dead_time" => {
                let time_constant_s = finite(
                    "plants.parameters.time_constant_s",
                    self.parameters.time_constant_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "first_order_dead_time requires time_constant_s".into(),
                        )
                    })?,
                )?;
                let dead_time_s = finite(
                    "plants.parameters.dead_time_s",
                    self.parameters.dead_time_s.unwrap_or(0.0),
                )?;
                if time_constant_s <= 0.0 || dead_time_s < 0.0 {
                    return invalid("FOPDT time constant must be > 0 and dead time >= 0");
                }
                PlantModelConfig::FirstOrderDeadTime {
                    gain: finite(
                        "plants.parameters.gain",
                        self.parameters.gain.unwrap_or(1.0),
                    )?,
                    time_constant_s,
                    input_bias: finite(
                        "plants.parameters.input_bias",
                        self.parameters.input_bias.unwrap_or(0.0),
                    )?,
                    output_bias: finite(
                        "plants.parameters.output_bias",
                        self.parameters.output_bias.unwrap_or(0.0),
                    )?,
                    dead_time_s,
                    initial_delayed_input: finite(
                        "plants.parameters.initial_delayed_input",
                        self.parameters.initial_delayed_input.unwrap_or(0.0),
                    )?,
                }
            }
            "integrator" => PlantModelConfig::Integrator {
                gain: finite(
                    "plants.parameters.gain",
                    self.parameters.gain.unwrap_or(1.0),
                )?,
                input_bias: finite(
                    "plants.parameters.input_bias",
                    self.parameters.input_bias.unwrap_or(0.0),
                )?,
                output_bias: finite(
                    "plants.parameters.output_bias",
                    self.parameters.output_bias.unwrap_or(0.0),
                )?,
            },
            "piecewise_linear" => {
                let input_points = self.parameters.input_points.ok_or_else(|| {
                    RuntimeError::InvalidConfig("piecewise_linear requires input_points".into())
                })?;
                let output_points = self.parameters.output_points.ok_or_else(|| {
                    RuntimeError::InvalidConfig("piecewise_linear requires output_points".into())
                })?;
                if input_points.len() < 2 || input_points.len() != output_points.len() {
                    return invalid("piecewise_linear point arrays must have equal length >= 2");
                }
                if input_points.iter().any(|value| !value.is_finite())
                    || output_points.iter().any(|value| !value.is_finite())
                    || input_points.windows(2).any(|pair| pair[0] >= pair[1])
                {
                    return invalid("piecewise_linear points must be finite and inputs increasing");
                }
                PlantModelConfig::PiecewiseLinear {
                    input_points,
                    output_points,
                }
            }
            "second_order" => {
                let natural_frequency = finite(
                    "plants.parameters.natural_frequency_rad_s",
                    self.parameters.natural_frequency_rad_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "second_order requires natural_frequency_rad_s".into(),
                        )
                    })?,
                )?;
                let damping_ratio = finite(
                    "plants.parameters.damping_ratio",
                    self.parameters.damping_ratio.ok_or_else(|| {
                        RuntimeError::InvalidConfig("second_order requires damping_ratio".into())
                    })?,
                )?;
                if natural_frequency <= 0.0 || damping_ratio < 0.0 {
                    return invalid("second_order frequency must be > 0 and damping >= 0");
                }
                PlantModelConfig::SecondOrder {
                    gain: finite(
                        "plants.parameters.gain",
                        self.parameters.gain.unwrap_or(1.0),
                    )?,
                    natural_frequency,
                    damping_ratio,
                    input_bias: finite(
                        "plants.parameters.input_bias",
                        self.parameters.input_bias.unwrap_or(0.0),
                    )?,
                    output_bias: finite(
                        "plants.parameters.output_bias",
                        self.parameters.output_bias.unwrap_or(0.0),
                    )?,
                }
            }
            "state_space" => {
                let a = self.parameters.a.ok_or_else(|| {
                    RuntimeError::InvalidConfig("state_space requires matrix a".into())
                })?;
                let b = self.parameters.b.ok_or_else(|| {
                    RuntimeError::InvalidConfig("state_space requires matrix b".into())
                })?;
                let c = self.parameters.c.ok_or_else(|| {
                    RuntimeError::InvalidConfig("state_space requires matrix c".into())
                })?;
                let d = self.parameters.d.ok_or_else(|| {
                    RuntimeError::InvalidConfig("state_space requires matrix d".into())
                })?;
                let n = a.len();
                let m = command_count;
                let p = feedback_count;
                if n == 0
                    || a.iter().any(|row| row.len() != n)
                    || b.len() != n
                    || b.iter().any(|row| row.len() != m)
                    || c.len() != p
                    || c.iter().any(|row| row.len() != n)
                    || d.len() != p
                    || d.iter().any(|row| row.len() != m)
                    || [&a, &b, &c, &d]
                        .into_iter()
                        .flat_map(|matrix| matrix.iter().flatten())
                        .any(|value| !value.is_finite())
                {
                    return invalid("state_space matrix dimensions or values are invalid");
                }
                PlantModelConfig::StateSpace { a, b, c, d }
            }
            other => return invalid(format!("unsupported plant model '{other}'")),
        };
        if !matches!(model, PlantModelConfig::StateSpace { .. })
            && (command_count != 1 || feedback_count != 1)
        {
            return invalid(format!(
                "plant model '{}' requires one command source and one feedback target",
                self.model
            ));
        }
        let expected_states = match &model {
            PlantModelConfig::StaticGain { .. } | PlantModelConfig::PiecewiseLinear { .. } => 0,
            PlantModelConfig::FirstOrder { .. }
            | PlantModelConfig::FirstOrderDeadTime { .. }
            | PlantModelConfig::Integrator { .. } => 1,
            PlantModelConfig::SecondOrder { .. } => 2,
            PlantModelConfig::StateSpace { a, .. } => a.len(),
        };
        let state = requested_state.unwrap_or_else(|| vec![0.0; expected_states]);
        if state.len() != expected_states || state.iter().any(|value| !value.is_finite()) {
            return invalid(format!(
                "plant model '{}' requires {expected_states} finite initial states",
                self.model
            ));
        }
        let step = self
            .step_ms
            .map(|value| checked_step("plants.step_ms", value))
            .transpose()?
            .unwrap_or(default_step);
        let initial_input = channel_setting(
            "plants.initial_input",
            self.initial_input,
            command_count,
            0.0,
        )?;
        let input_scale =
            channel_setting("plants.input_scale", self.input_scale, command_count, 1.0)?;
        let input_offset =
            channel_setting("plants.input_offset", self.input_offset, command_count, 0.0)?;
        let output_scale = channel_setting(
            "plants.output_scale",
            self.output_scale,
            feedback_count,
            1.0,
        )?;
        let output_offset = channel_setting(
            "plants.output_offset",
            self.output_offset,
            feedback_count,
            0.0,
        )?;
        if command_count > 1 && self.actuator.is_some() {
            return invalid("plants.actuator is supported only for scalar-input models");
        }
        let mut actuators = Vec::with_capacity(command_count);
        for (channel, initial) in initial_input.iter().copied().enumerate() {
            let section = if channel == 0 {
                self.actuator.take().unwrap_or_default()
            } else {
                ActuatorSection::default()
            };
            actuators.push(section.into_config(initial)?);
        }
        let uncertainties = self
            .uncertainties
            .into_iter()
            .map(UncertaintySection::into_config)
            .collect::<Result<Vec<_>, _>>()?;
        for uncertainty in &uncertainties {
            uncertainty.validate_channel(command_count, state.len(), feedback_count)?;
        }
        let mut uncertainty_ids = std::collections::HashSet::new();
        if uncertainties
            .iter()
            .any(|effect| !uncertainty_ids.insert(effect.id_for_validation()))
        {
            return invalid("uncertainty ids must be unique within a plant");
        }
        Ok(PlantConfig {
            id: self.id,
            model,
            command_sources,
            feedback_targets,
            step,
            initial_state: state,
            initial_input,
            input_scale,
            input_offset,
            output_scale,
            output_offset,
            actuators,
            uncertainties,
            master_seed,
            enabled: self.enabled.unwrap_or(true),
            events: Vec::new(),
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct PlantSystem {
    plants: Vec<PlantInstance>,
    max_substeps_per_cycle: usize,
    last_time: Duration,
    next_trace_sequence: u64,
}

impl PlantSystem {
    pub(super) fn new(config: &PlantSystemConfig) -> Self {
        Self {
            plants: config
                .plants
                .iter()
                .cloned()
                .map(|plant| PlantInstance::new(plant, config.trace_capacity))
                .collect(),
            max_substeps_per_cycle: config.max_substeps_per_cycle,
            last_time: Duration::ZERO,
            next_trace_sequence: 0,
        }
    }

    pub(super) fn publish(&mut self, runtime: &mut Runtime) -> Result<(), RuntimeError> {
        for plant in &mut self.plants {
            plant.publish(runtime)?;
        }
        Ok(())
    }

    pub(super) fn advance(&mut self, now: Duration, runtime: &Runtime) -> Result<(), RuntimeError> {
        let elapsed = now.as_nanos().saturating_sub(self.last_time.as_nanos());
        self.last_time = now;
        for plant in &mut self.plants {
            plant.advance(
                elapsed,
                self.max_substeps_per_cycle,
                &mut self.next_trace_sequence,
                runtime,
            )?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PlantInstance {
    config: PlantConfig,
    state: Vec<f64>,
    accumulator_nanos: i64,
    queued_feedback: Vec<f64>,
    feedback_pending: bool,
    model_time_nanos: i64,
    input_history: VecDeque<(i64, f64)>,
    actuators: Vec<ActuatorState>,
    uncertainties: Vec<UncertaintyState>,
    event_cursor: usize,
    parameter_overrides: Vec<Option<f64>>,
    sensor_bias: Vec<f64>,
    last_substeps: usize,
    latest_command: Vec<f64>,
    latest_applied_input: Vec<f64>,
    latest_ideal_output: Vec<f64>,
    latest_measured_feedback: Vec<f64>,
    published_feedback: Vec<f64>,
    fired_events: Vec<&'static str>,
    last_fault: Option<String>,
    trace: TraceBuffer,
    effective_model: PlantModelConfig,
    effective_commands: Vec<f64>,
    model_workspace: ModelWorkspace,
    checkpoint: PlantCheckpoint,
}

#[derive(Debug, Clone)]
struct PlantCheckpoint {
    state: Vec<f64>,
    queued_feedback: Vec<f64>,
    feedback_pending: bool,
    model_time_nanos: i64,
    input_history: VecDeque<(i64, f64)>,
    actuators: Vec<ActuatorState>,
    uncertainties: Vec<UncertaintyState>,
    event_cursor: usize,
    parameter_overrides: Vec<Option<f64>>,
    sensor_bias: Vec<f64>,
    latest_command: Vec<f64>,
    latest_applied_input: Vec<f64>,
    latest_ideal_output: Vec<f64>,
    latest_measured_feedback: Vec<f64>,
}
