//! Dynamic process models for simulation-mode process-image feedback.

use serde::Deserialize;

use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::value::Duration;
use crate::Runtime;

#[cfg(test)]
#[path = "plant/cascade_tests.rs"]
mod cascade_tests;
#[cfg(test)]
#[path = "plant/course_tanks_tests.rs"]
mod course_tanks_tests;
#[path = "plant/effects.rs"]
mod effects;
#[path = "plant/events.rs"]
mod events;
#[path = "plant/execution.rs"]
mod execution;
#[path = "plant/input_history.rs"]
mod input_history;
#[path = "plant/model.rs"]
mod model;
#[path = "plant/numeric_io.rs"]
mod numeric_io;
#[cfg(test)]
#[path = "plant/resource_tests.rs"]
mod resource_tests;
#[path = "plant/state_space_delay.rs"]
mod state_space_delay;
#[path = "plant/state_space_parameters.rs"]
mod state_space_parameters;
#[path = "plant/status.rs"]
mod status;
#[path = "plant/uncertainty.rs"]
mod uncertainty;
#[path = "plant/validation.rs"]
mod validation;
#[path = "plant/validity.rs"]
mod validity;

use effects::{ActuatorConfig, ActuatorSection, ActuatorState};
pub(super) use events::PlantEventSection;
use events::{PlantEventAction, PlantEventConfig};
use input_history::InputHistory;
use model::{
    advance_delayed_integrator, advance_fopdt, model_output_into, rk4_gravity_tank,
    rk4_gravity_tank_cascade, rk4_robot_joint, rk4_second_order, rk4_state_space, ModelWorkspace,
    PlantModelConfig,
};
use numeric_io::read_numeric;
use status::TraceBuffer;
use uncertainty::{UncertaintyConfig, UncertaintySection, UncertaintyState};
use validation::{
    apply_parameter_overrides, apply_parameter_uncertainties, channel_setting, checked_step,
    finite, invalid, model_supports_parameter, parse_addresses, valid_id,
};
use validity::{ValidityConfig, ValiditySection};

const DEFAULT_STEP_MS: i64 = 10;
const DEFAULT_MAX_SUBSTEPS: usize = 10_000;

pub(crate) const SUPPORTED_MODEL_KINDS: &[&str] = &[
    "static_gain",
    "first_order",
    "first_order_dead_time",
    "integrator",
    "integrator_dead_time",
    "gravity_tank",
    "gravity_tank_cascade",
    "piecewise_linear",
    "robot_joint",
    "second_order",
    "state_space",
    "discrete_state_space",
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
    discrete_state_space: bool,
    input_delays_nanos: Vec<i64>,
    command_sources: Vec<IoAddress>,
    feedback_targets: Vec<IoAddress>,
    actuator_feedback_targets: Vec<Option<IoAddress>>,
    step: Duration,
    initial_state: Vec<f64>,
    initial_input: Vec<f64>,
    input_scale: Vec<f64>,
    input_offset: Vec<f64>,
    output_scale: Vec<f64>,
    output_offset: Vec<f64>,
    actuators: Vec<ActuatorConfig>,
    uncertainties: Vec<UncertaintyConfig>,
    validity: ValidityConfig,
    master_seed: u64,
    enabled: bool,
    events: Vec<PlantEventConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ActuatorFeedbackTarget {
    address: Option<String>,
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
    actuator_feedback_targets: Option<Vec<ActuatorFeedbackTarget>>,
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
    actuators: Option<Vec<ActuatorSection>>,
    #[serde(default)]
    uncertainties: Vec<UncertaintySection>,
    validity: Option<ValiditySection>,
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
    input_delays_s: Option<Vec<f64>>,
    dead_time_s: Option<f64>,
    initial_delayed_input: Option<f64>,
    area_m2: Option<f64>,
    outflow_coefficient_m2_sqrt_m_per_s: Option<f64>,
    inertia_kg_m2: Option<f64>,
    gravity_torque_nm: Option<f64>,
    viscous_friction_nm_s_per_rad: Option<f64>,
    coulomb_friction_nm: Option<f64>,
    friction_smoothing_rad_s: Option<f64>,
}

impl PlantParametersSection {
    fn validate_for_model(&self, model: &str) -> Result<(), RuntimeError> {
        let allowed: &[&str] = match model {
            "static_gain" | "integrator" => &["gain", "input_bias", "output_bias"],
            "integrator_dead_time" => &[
                "gain",
                "input_bias",
                "output_bias",
                "dead_time_s",
                "initial_delayed_input",
            ],
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
            "state_space" => &["a", "b", "c", "d", "input_delays_s"],
            "discrete_state_space" => &["a", "b", "c", "d"],
            "gravity_tank" | "gravity_tank_cascade" => {
                &["area_m2", "outflow_coefficient_m2_sqrt_m_per_s"]
            }
            "robot_joint" => &[
                "inertia_kg_m2",
                "gravity_torque_nm",
                "viscous_friction_nm_s_per_rad",
                "coulomb_friction_nm",
                "friction_smoothing_rad_s",
            ],
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
            ("input_delays_s", self.input_delays_s.is_some()),
            ("dead_time_s", self.dead_time_s.is_some()),
            (
                "initial_delayed_input",
                self.initial_delayed_input.is_some(),
            ),
            ("area_m2", self.area_m2.is_some()),
            (
                "outflow_coefficient_m2_sqrt_m_per_s",
                self.outflow_coefficient_m2_sqrt_m_per_s.is_some(),
            ),
            ("inertia_kg_m2", self.inertia_kg_m2.is_some()),
            ("gravity_torque_nm", self.gravity_torque_nm.is_some()),
            (
                "viscous_friction_nm_s_per_rad",
                self.viscous_friction_nm_s_per_rad.is_some(),
            ),
            ("coulomb_friction_nm", self.coulomb_friction_nm.is_some()),
            (
                "friction_smoothing_rad_s",
                self.friction_smoothing_rad_s.is_some(),
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
            plant
                .feedback_targets
                .iter()
                .chain(plant.actuator_feedback_targets.iter().flatten())
                .any(|target| {
                    other
                        .feedback_targets
                        .iter()
                        .chain(other.actuator_feedback_targets.iter().flatten())
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
                .chain(plant.actuator_feedback_targets.iter().flatten())
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
        let actuator_feedback_targets = self.actuator_feedback_targets.unwrap_or_else(|| {
            (0..command_count)
                .map(|_| ActuatorFeedbackTarget { address: None })
                .collect()
        });
        if actuator_feedback_targets.len() != command_count {
            return invalid("actuator feedback targets must match ordered command channels");
        }
        let actuator_feedback_targets = actuator_feedback_targets
            .into_iter()
            .map(|target| {
                target
                    .address
                    .map(|target| {
                        let parsed =
                            parse_addresses(&[target], IoArea::Input, "actuator feedback")?;
                        Ok(parsed[0].clone())
                    })
                    .transpose()
            })
            .collect::<Result<Vec<_>, RuntimeError>>()?;
        let published = feedback_targets
            .iter()
            .chain(actuator_feedback_targets.iter().flatten())
            .collect::<Vec<_>>();
        if published.iter().enumerate().any(|(index, target)| {
            published[..index]
                .iter()
                .any(|other| super::addresses_overlap(other, target))
        }) {
            return invalid("plant feedback/actuator targets overlap");
        }
        let feedback_count = feedback_targets.len();
        let input_delays_nanos = if self.model == "state_space" {
            state_space_parameters::input_delays(
                self.parameters.input_delays_s.take(),
                command_count,
            )?
        } else {
            Vec::new()
        };
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
                        self.parameters
                            .initial_delayed_input
                            .unwrap_or(self.parameters.input_bias.unwrap_or(0.0)),
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
            "integrator_dead_time" => {
                let dead_time_s = finite(
                    "plants.parameters.dead_time_s",
                    self.parameters.dead_time_s.unwrap_or(0.0),
                )?;
                if dead_time_s < 0.0 {
                    return invalid("delayed integrator dead time must be >= 0");
                }
                PlantModelConfig::IntegratorDeadTime {
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
                    dead_time_s,
                    initial_delayed_input: finite(
                        "plants.parameters.initial_delayed_input",
                        self.parameters.initial_delayed_input.ok_or_else(|| {
                            RuntimeError::InvalidConfig(
                                "integrator_dead_time requires initial_delayed_input".into(),
                            )
                        })?,
                    )?,
                }
            }
            "gravity_tank" | "gravity_tank_cascade" => {
                let area_m2 = finite(
                    "plants.parameters.area_m2",
                    self.parameters.area_m2.ok_or_else(|| {
                        RuntimeError::InvalidConfig("gravity_tank requires area_m2".into())
                    })?,
                )?;
                let outflow_coefficient = finite(
                    "plants.parameters.outflow_coefficient_m2_sqrt_m_per_s",
                    self.parameters
                        .outflow_coefficient_m2_sqrt_m_per_s
                        .ok_or_else(|| {
                            RuntimeError::InvalidConfig(
                                "gravity_tank requires outflow_coefficient_m2_sqrt_m_per_s".into(),
                            )
                        })?,
                )?;
                if area_m2 <= 0.0 || outflow_coefficient < 0.0 {
                    return invalid(
                        "gravity_tank area must be > 0 and outflow coefficient must be >= 0",
                    );
                }
                if self.model == "gravity_tank_cascade" {
                    PlantModelConfig::GravityTankCascade {
                        area_m2,
                        outflow_coefficient,
                    }
                } else {
                    PlantModelConfig::GravityTank {
                        area_m2,
                        outflow_coefficient,
                    }
                }
            }
            "robot_joint" => {
                let inertia = finite(
                    "plants.parameters.inertia_kg_m2",
                    self.parameters.inertia_kg_m2.ok_or_else(|| {
                        RuntimeError::InvalidConfig("robot_joint requires inertia_kg_m2".into())
                    })?,
                )?;
                let gravity_torque = finite(
                    "plants.parameters.gravity_torque_nm",
                    self.parameters.gravity_torque_nm.unwrap_or(0.0),
                )?;
                let viscous_friction = finite(
                    "plants.parameters.viscous_friction_nm_s_per_rad",
                    self.parameters.viscous_friction_nm_s_per_rad.unwrap_or(0.0),
                )?;
                let coulomb_friction = finite(
                    "plants.parameters.coulomb_friction_nm",
                    self.parameters.coulomb_friction_nm.unwrap_or(0.0),
                )?;
                let friction_smoothing = finite(
                    "plants.parameters.friction_smoothing_rad_s",
                    self.parameters.friction_smoothing_rad_s.ok_or_else(|| {
                        RuntimeError::InvalidConfig(
                            "robot_joint requires friction_smoothing_rad_s".into(),
                        )
                    })?,
                )?;
                if inertia <= 0.0
                    || gravity_torque < 0.0
                    || viscous_friction < 0.0
                    || coulomb_friction < 0.0
                    || friction_smoothing <= 0.0
                {
                    return invalid(
                        "robot_joint physical parameters are outside their valid domain",
                    );
                }
                PlantModelConfig::RobotJoint {
                    inertia,
                    gravity_torque,
                    viscous_friction,
                    coulomb_friction,
                    friction_smoothing,
                }
            }
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
            "state_space" | "discrete_state_space" => {
                state_space_parameters::model(&mut self.parameters, command_count, feedback_count)?
            }
            other => return invalid(format!("unsupported plant model '{other}'")),
        };
        match &model {
            PlantModelConfig::StateSpace { .. } => {}
            PlantModelConfig::RobotJoint { .. } | PlantModelConfig::GravityTankCascade { .. }
                if command_count == 1 && feedback_count == 2 => {}
            PlantModelConfig::GravityTankCascade { .. } => {
                return invalid(
                    "gravity_tank_cascade requires one command and two feedback targets",
                );
            }
            PlantModelConfig::RobotJoint { .. } => {
                return invalid(
                    "plant model 'robot_joint' requires one command and two feedback targets",
                );
            }
            _ if command_count != 1 || feedback_count != 1 => {
                return invalid(format!(
                    "plant model '{}' requires one command source and one feedback target",
                    self.model
                ));
            }
            _ => {}
        }
        let expected_states = match &model {
            PlantModelConfig::StaticGain { .. } | PlantModelConfig::PiecewiseLinear { .. } => 0,
            PlantModelConfig::FirstOrder { .. }
            | PlantModelConfig::FirstOrderDeadTime { .. }
            | PlantModelConfig::Integrator { .. }
            | PlantModelConfig::IntegratorDeadTime { .. } => 1,
            PlantModelConfig::GravityTank { .. } => 1,
            PlantModelConfig::RobotJoint { .. } | PlantModelConfig::GravityTankCascade { .. } => 2,
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
        InputHistory::validate_configuration(
            &self.id,
            &model,
            self.model == "discrete_state_space",
            &input_delays_nanos,
            step.as_nanos(),
        )?;
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
        let actuators = effects::configured_actuators(
            self.actuator.take(),
            self.actuators.take(),
            &initial_input,
        )?;
        let validity = self.validity.unwrap_or_default().into_config(
            command_count,
            state.len(),
            feedback_count,
        )?;
        let initial_applied = actuators
            .iter()
            .map(ActuatorConfig::initial_applied)
            .collect::<Vec<_>>();
        validity.validate_initial(&initial_applied, &state)?;
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
        if actuator_feedback_targets
            .iter()
            .enumerate()
            .any(|(channel, target)| target.is_some() && input_scale[channel] == 0.0)
        {
            return invalid("applied actuator feedback cannot invert a zero command scale");
        }
        let discrete_state_space = self.model == "discrete_state_space";
        Ok(PlantConfig {
            id: self.id,
            model,
            discrete_state_space,
            input_delays_nanos,
            command_sources,
            feedback_targets,
            actuator_feedback_targets,
            step,
            initial_state: state,
            initial_input,
            input_scale,
            input_offset,
            output_scale,
            output_offset,
            actuators,
            uncertainties,
            validity,
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
    input_history: InputHistory,
    actuators: Vec<ActuatorState>,
    uncertainties: Vec<UncertaintyState>,
    event_cursor: usize,
    parameter_overrides: Vec<Option<f64>>,
    sensor_bias: Vec<f64>,
    last_substeps: usize,
    held_command: Vec<f64>,
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
    delayed_inputs: Vec<f64>,
    model_workspace: ModelWorkspace,
    checkpoint: PlantCheckpoint,
}

#[derive(Debug, Clone)]
struct PlantCheckpoint {
    state: Vec<f64>,
    queued_feedback: Vec<f64>,
    feedback_pending: bool,
    model_time_nanos: i64,
    input_history: InputHistory,
    actuators: Vec<ActuatorState>,
    uncertainties: Vec<UncertaintyState>,
    event_cursor: usize,
    parameter_overrides: Vec<Option<f64>>,
    sensor_bias: Vec<f64>,
    held_command: Vec<f64>,
    latest_command: Vec<f64>,
    latest_applied_input: Vec<f64>,
    latest_ideal_output: Vec<f64>,
    latest_measured_feedback: Vec<f64>,
}
