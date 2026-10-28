//! Validated, time-ordered scenario actions for plant instances.

use serde::Deserialize;

use crate::error::RuntimeError;

#[derive(Debug, Clone)]
pub(super) struct PlantEventConfig {
    pub(super) at_nanos: i64,
    pub(super) action: PlantEventAction,
}

#[derive(Debug, Clone)]
pub(super) enum PlantEventAction {
    SetState { channel: usize, value: f64 },
    AddState { channel: usize, value: f64 },
    SetParameter { parameter: String, value: f64 },
    SetSensorBias { channel: usize, value: f64 },
    StickActuator,
    ReleaseActuator,
    ResetPlant,
}

impl PlantEventAction {
    pub(super) fn kind(&self) -> &'static str {
        match self {
            Self::SetState { .. } => "set_state",
            Self::AddState { .. } => "add_state",
            Self::SetParameter { .. } => "set_parameter",
            Self::SetSensorBias { .. } => "set_sensor_bias",
            Self::StickActuator => "stick_actuator",
            Self::ReleaseActuator => "release_actuator",
            Self::ResetPlant => "reset_plant",
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct PlantEventSection {
    pub(super) at_ms: u64,
    pub(super) plant: String,
    kind: String,
    channel: Option<usize>,
    value: Option<f64>,
    parameter: Option<String>,
}

impl PlantEventSection {
    pub(super) fn into_config(
        self,
        state_count: usize,
        output_count: usize,
        scalar_actuator: bool,
    ) -> Result<PlantEventConfig, RuntimeError> {
        if self.at_ms > (i64::MAX / 1_000_000) as u64 {
            return invalid("plant event at_ms exceeds runtime duration");
        }
        let state_event = |add: bool| -> Result<PlantEventAction, RuntimeError> {
            let channel = self.channel.ok_or_else(|| {
                RuntimeError::InvalidConfig("state event requires channel".into())
            })?;
            if channel >= state_count {
                return invalid("plant event state channel is out of range");
            }
            let value = self
                .value
                .ok_or_else(|| RuntimeError::InvalidConfig("state event requires value".into()))?;
            if !value.is_finite() || self.parameter.is_some() {
                return invalid("state event fields are invalid for its kind");
            }
            Ok(if add {
                PlantEventAction::AddState { channel, value }
            } else {
                PlantEventAction::SetState { channel, value }
            })
        };
        let action = match self.kind.as_str() {
            "set_state" => state_event(false)?,
            "add_state" => state_event(true)?,
            "set_parameter" => {
                if self.channel.is_some() {
                    return invalid("set_parameter must not declare channel");
                }
                PlantEventAction::SetParameter {
                    parameter: self.parameter.ok_or_else(|| {
                        RuntimeError::InvalidConfig("set_parameter requires parameter".into())
                    })?,
                    value: checked_value(self.value, "set_parameter")?,
                }
            }
            "set_sensor_bias" => {
                let channel = self.channel.ok_or_else(|| {
                    RuntimeError::InvalidConfig("set_sensor_bias requires channel".into())
                })?;
                if channel >= output_count || self.parameter.is_some() {
                    return invalid("set_sensor_bias channel is out of range");
                }
                PlantEventAction::SetSensorBias {
                    channel,
                    value: checked_value(self.value, "set_sensor_bias")?,
                }
            }
            "stick_actuator" | "release_actuator" | "reset_plant" => {
                if self.channel.is_some() || self.value.is_some() || self.parameter.is_some() {
                    return invalid(format!("{} accepts no action fields", self.kind));
                }
                match self.kind.as_str() {
                    "stick_actuator" if scalar_actuator => PlantEventAction::StickActuator,
                    "release_actuator" if scalar_actuator => PlantEventAction::ReleaseActuator,
                    "stick_actuator" | "release_actuator" => {
                        return invalid("actuator events require a scalar-input plant");
                    }
                    _ => PlantEventAction::ResetPlant,
                }
            }
            other => return invalid(format!("unsupported plant event kind '{other}'")),
        };
        Ok(PlantEventConfig {
            at_nanos: self.at_ms as i64 * 1_000_000,
            action,
        })
    }
}

fn checked_value(value: Option<f64>, kind: &str) -> Result<f64, RuntimeError> {
    let value = value
        .ok_or_else(|| RuntimeError::InvalidConfig(format!("{kind} requires value").into()))?;
    if !value.is_finite() {
        return invalid(format!("{kind} value must be finite"));
    }
    Ok(value)
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}
