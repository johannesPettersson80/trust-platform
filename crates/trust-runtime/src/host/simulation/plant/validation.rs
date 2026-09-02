//! Shared validation for scalar plant configuration values and I/O bindings.

use crate::error::RuntimeError;
use crate::io::IoAddress;
use crate::memory::IoArea;
use crate::value::Duration;

use super::uncertainty::UncertaintyState;
use super::PlantModelConfig;

pub(super) fn parse_addresses(
    values: &[String],
    expected_area: IoArea,
    role: &str,
) -> Result<Vec<IoAddress>, RuntimeError> {
    values
        .iter()
        .map(|text| {
            let address = IoAddress::parse(text)?;
            if address.area != expected_area {
                return invalid(format!("plant {role} address has wrong area: {text}"));
            }
            Ok(address)
        })
        .collect()
}

pub(super) fn checked_step(name: &str, millis: u64) -> Result<Duration, RuntimeError> {
    if millis == 0 || millis > (i64::MAX / 1_000_000) as u64 {
        return invalid(format!("{name} must fit a positive runtime duration"));
    }
    Ok(Duration::from_millis(millis as i64))
}

pub(super) fn channel_setting(
    name: &str,
    values: Option<Vec<f64>>,
    channels: usize,
    default: f64,
) -> Result<Vec<f64>, RuntimeError> {
    let values = values.unwrap_or_else(|| vec![default; channels]);
    if values.len() != channels {
        return invalid(format!(
            "{name} must contain one value for each of the {channels} channels"
        ));
    }
    values
        .into_iter()
        .map(|value| finite(name, value))
        .collect()
}

pub(super) fn valid_id(value: &str) -> bool {
    let mut chars = value.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || ch == '_' || ch == '-')
}

pub(super) fn finite(name: &str, value: f64) -> Result<f64, RuntimeError> {
    if !value.is_finite() {
        return invalid(format!("{name} must be finite"));
    }
    Ok(value)
}

pub(super) fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}

pub(super) fn model_supports_parameter(model: &PlantModelConfig, parameter: &str) -> bool {
    parameter_value(model, parameter).is_some()
}

pub(super) fn apply_parameter_overrides(
    model: &mut PlantModelConfig,
    events: &[super::events::PlantEventConfig],
    overrides: &[Option<f64>],
) {
    for (event, value) in events.iter().zip(overrides) {
        let (super::events::PlantEventAction::SetParameter { parameter, .. }, Some(value)) =
            (&event.action, value)
        else {
            continue;
        };
        let changed = set_parameter(model, parameter, *value);
        debug_assert!(
            changed,
            "validated parameter override must remain addressable"
        );
    }
}

pub(super) fn apply_parameter_uncertainties(
    model: &mut PlantModelConfig,
    uncertainties: &mut [UncertaintyState],
    time_s: f64,
) -> Result<(), RuntimeError> {
    for effect in uncertainties {
        let Some(name) = effect.parameter_name() else {
            continue;
        };
        let current = parameter_value(model, name).ok_or_else(|| {
            RuntimeError::SimulationFault(format!("plant parameter '{name}' disappeared").into())
        })?;
        let effective = effect.apply_parameter(current, time_s);
        let name = effect
            .parameter_name()
            .expect("parameter-target effect retains its validated parameter");
        if !set_parameter(model, name, effective) {
            return Err(RuntimeError::SimulationFault(
                format!("plant parameter '{name}' disappeared").into(),
            ));
        }
    }
    validate_effective_model(model)
}

fn parameter_value(model: &PlantModelConfig, name: &str) -> Option<f64> {
    match (model, name) {
        (PlantModelConfig::StaticGain { gain, .. }, "gain")
        | (PlantModelConfig::FirstOrder { gain, .. }, "gain")
        | (PlantModelConfig::FirstOrderDeadTime { gain, .. }, "gain")
        | (PlantModelConfig::Integrator { gain, .. }, "gain")
        | (PlantModelConfig::SecondOrder { gain, .. }, "gain") => Some(*gain),
        (PlantModelConfig::StaticGain { input_bias, .. }, "input_bias")
        | (PlantModelConfig::FirstOrder { input_bias, .. }, "input_bias")
        | (PlantModelConfig::FirstOrderDeadTime { input_bias, .. }, "input_bias")
        | (PlantModelConfig::Integrator { input_bias, .. }, "input_bias")
        | (PlantModelConfig::SecondOrder { input_bias, .. }, "input_bias") => Some(*input_bias),
        (PlantModelConfig::StaticGain { output_bias, .. }, "output_bias")
        | (PlantModelConfig::FirstOrder { output_bias, .. }, "output_bias")
        | (PlantModelConfig::FirstOrderDeadTime { output_bias, .. }, "output_bias")
        | (PlantModelConfig::Integrator { output_bias, .. }, "output_bias")
        | (PlantModelConfig::SecondOrder { output_bias, .. }, "output_bias") => Some(*output_bias),
        (
            PlantModelConfig::FirstOrder {
                time_constant_s, ..
            },
            "time_constant_s",
        )
        | (
            PlantModelConfig::FirstOrderDeadTime {
                time_constant_s, ..
            },
            "time_constant_s",
        ) => Some(*time_constant_s),
        (PlantModelConfig::FirstOrderDeadTime { dead_time_s, .. }, "dead_time_s") => {
            Some(*dead_time_s)
        }
        (
            PlantModelConfig::FirstOrderDeadTime {
                initial_delayed_input,
                ..
            },
            "initial_delayed_input",
        ) => Some(*initial_delayed_input),
        (
            PlantModelConfig::SecondOrder {
                natural_frequency, ..
            },
            "natural_frequency_rad_s",
        ) => Some(*natural_frequency),
        (PlantModelConfig::SecondOrder { damping_ratio, .. }, "damping_ratio") => {
            Some(*damping_ratio)
        }
        (PlantModelConfig::GravityTank { area_m2, .. }, "area_m2") => Some(*area_m2),
        (
            PlantModelConfig::GravityTank {
                outflow_coefficient,
                ..
            },
            "outflow_coefficient_m2_sqrt_m_per_s",
        ) => Some(*outflow_coefficient),
        (PlantModelConfig::RobotJoint { inertia, .. }, "inertia_kg_m2") => Some(*inertia),
        (PlantModelConfig::RobotJoint { gravity_torque, .. }, "gravity_torque_nm") => {
            Some(*gravity_torque)
        }
        (
            PlantModelConfig::RobotJoint {
                viscous_friction, ..
            },
            "viscous_friction_nm_s_per_rad",
        ) => Some(*viscous_friction),
        (
            PlantModelConfig::RobotJoint {
                coulomb_friction, ..
            },
            "coulomb_friction_nm",
        ) => Some(*coulomb_friction),
        (
            PlantModelConfig::RobotJoint {
                friction_smoothing, ..
            },
            "friction_smoothing_rad_s",
        ) => Some(*friction_smoothing),
        (PlantModelConfig::StateSpace { a, b, c, d }, _) => {
            let (matrix, row, column) = parse_matrix_parameter(name)?;
            let selected = match matrix {
                'a' => a,
                'b' => b,
                'c' => c,
                'd' => d,
                _ => return None,
            };
            selected.get(row)?.get(column).copied()
        }
        _ => None,
    }
}

fn set_parameter(model: &mut PlantModelConfig, name: &str, value: f64) -> bool {
    match (model, name) {
        (PlantModelConfig::StaticGain { gain, .. }, "gain")
        | (PlantModelConfig::FirstOrder { gain, .. }, "gain")
        | (PlantModelConfig::FirstOrderDeadTime { gain, .. }, "gain")
        | (PlantModelConfig::Integrator { gain, .. }, "gain")
        | (PlantModelConfig::SecondOrder { gain, .. }, "gain") => *gain = value,
        (PlantModelConfig::StaticGain { input_bias, .. }, "input_bias")
        | (PlantModelConfig::FirstOrder { input_bias, .. }, "input_bias")
        | (PlantModelConfig::FirstOrderDeadTime { input_bias, .. }, "input_bias")
        | (PlantModelConfig::Integrator { input_bias, .. }, "input_bias")
        | (PlantModelConfig::SecondOrder { input_bias, .. }, "input_bias") => *input_bias = value,
        (PlantModelConfig::StaticGain { output_bias, .. }, "output_bias")
        | (PlantModelConfig::FirstOrder { output_bias, .. }, "output_bias")
        | (PlantModelConfig::FirstOrderDeadTime { output_bias, .. }, "output_bias")
        | (PlantModelConfig::Integrator { output_bias, .. }, "output_bias")
        | (PlantModelConfig::SecondOrder { output_bias, .. }, "output_bias") => {
            *output_bias = value;
        }
        (
            PlantModelConfig::FirstOrder {
                time_constant_s, ..
            },
            "time_constant_s",
        )
        | (
            PlantModelConfig::FirstOrderDeadTime {
                time_constant_s, ..
            },
            "time_constant_s",
        ) => *time_constant_s = value,
        (PlantModelConfig::FirstOrderDeadTime { dead_time_s, .. }, "dead_time_s") => {
            *dead_time_s = value;
        }
        (
            PlantModelConfig::FirstOrderDeadTime {
                initial_delayed_input,
                ..
            },
            "initial_delayed_input",
        ) => *initial_delayed_input = value,
        (
            PlantModelConfig::SecondOrder {
                natural_frequency, ..
            },
            "natural_frequency_rad_s",
        ) => *natural_frequency = value,
        (PlantModelConfig::SecondOrder { damping_ratio, .. }, "damping_ratio") => {
            *damping_ratio = value;
        }
        (PlantModelConfig::GravityTank { area_m2, .. }, "area_m2") => *area_m2 = value,
        (
            PlantModelConfig::GravityTank {
                outflow_coefficient,
                ..
            },
            "outflow_coefficient_m2_sqrt_m_per_s",
        ) => *outflow_coefficient = value,
        (PlantModelConfig::RobotJoint { inertia, .. }, "inertia_kg_m2") => *inertia = value,
        (PlantModelConfig::RobotJoint { gravity_torque, .. }, "gravity_torque_nm") => {
            *gravity_torque = value;
        }
        (
            PlantModelConfig::RobotJoint {
                viscous_friction, ..
            },
            "viscous_friction_nm_s_per_rad",
        ) => *viscous_friction = value,
        (
            PlantModelConfig::RobotJoint {
                coulomb_friction, ..
            },
            "coulomb_friction_nm",
        ) => {
            *coulomb_friction = value;
        }
        (
            PlantModelConfig::RobotJoint {
                friction_smoothing, ..
            },
            "friction_smoothing_rad_s",
        ) => *friction_smoothing = value,
        (PlantModelConfig::StateSpace { a, b, c, d }, _) => {
            let Some((matrix, row, column)) = parse_matrix_parameter(name) else {
                return false;
            };
            let selected = match matrix {
                'a' => a,
                'b' => b,
                'c' => c,
                'd' => d,
                _ => return false,
            };
            let Some(element) = selected.get_mut(row).and_then(|line| line.get_mut(column)) else {
                return false;
            };
            *element = value;
        }
        _ => return false,
    }
    true
}

fn parse_matrix_parameter(name: &str) -> Option<(char, usize, usize)> {
    let bytes = name.as_bytes();
    if bytes.len() < 6 || !matches!(bytes[0], b'a' | b'b' | b'c' | b'd') || bytes[1] != b'[' {
        return None;
    }
    let body = name.strip_suffix(']')?.get(2..)?;
    let (row, column) = body.split_once(',')?;
    if row.is_empty()
        || column.is_empty()
        || row.contains(char::is_whitespace)
        || column.contains(char::is_whitespace)
    {
        return None;
    }
    Some((bytes[0] as char, row.parse().ok()?, column.parse().ok()?))
}

fn validate_effective_model(model: &PlantModelConfig) -> Result<(), RuntimeError> {
    let valid = match model {
        PlantModelConfig::StaticGain {
            gain,
            input_bias,
            output_bias,
        }
        | PlantModelConfig::Integrator {
            gain,
            input_bias,
            output_bias,
        } => [gain, input_bias, output_bias]
            .into_iter()
            .all(|value| value.is_finite()),
        PlantModelConfig::FirstOrder {
            gain,
            time_constant_s,
            input_bias,
            output_bias,
        } => {
            [gain, time_constant_s, input_bias, output_bias]
                .into_iter()
                .all(|value| value.is_finite())
                && *time_constant_s > 0.0
        }
        PlantModelConfig::FirstOrderDeadTime {
            gain,
            time_constant_s,
            input_bias,
            output_bias,
            dead_time_s,
            initial_delayed_input,
        } => {
            [
                gain,
                time_constant_s,
                input_bias,
                output_bias,
                dead_time_s,
                initial_delayed_input,
            ]
            .into_iter()
            .all(|value| value.is_finite())
                && *time_constant_s > 0.0
                && *dead_time_s >= 0.0
        }
        PlantModelConfig::SecondOrder {
            gain,
            natural_frequency,
            damping_ratio,
            input_bias,
            output_bias,
        } => {
            [
                gain,
                natural_frequency,
                damping_ratio,
                input_bias,
                output_bias,
            ]
            .into_iter()
            .all(|value| value.is_finite())
                && *natural_frequency > 0.0
                && *damping_ratio >= 0.0
        }
        PlantModelConfig::PiecewiseLinear { .. } => true,
        PlantModelConfig::GravityTank {
            area_m2,
            outflow_coefficient,
        } => {
            area_m2.is_finite()
                && *area_m2 > 0.0
                && outflow_coefficient.is_finite()
                && *outflow_coefficient >= 0.0
        }
        PlantModelConfig::RobotJoint {
            inertia,
            gravity_torque,
            viscous_friction,
            coulomb_friction,
            friction_smoothing,
        } => {
            inertia.is_finite()
                && *inertia > 0.0
                && gravity_torque.is_finite()
                && *gravity_torque >= 0.0
                && viscous_friction.is_finite()
                && *viscous_friction >= 0.0
                && coulomb_friction.is_finite()
                && *coulomb_friction >= 0.0
                && friction_smoothing.is_finite()
                && *friction_smoothing > 0.0
        }
        PlantModelConfig::StateSpace { a, b, c, d } => [a, b, c, d]
            .into_iter()
            .flat_map(|matrix| matrix.iter().flatten())
            .all(|value| value.is_finite()),
    };
    if valid {
        Ok(())
    } else {
        Err(RuntimeError::SimulationFault(
            "effective plant parameter left its valid domain".into(),
        ))
    }
}
