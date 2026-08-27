//! Physical command-path effects applied before a plant model.

use serde::Deserialize;

use crate::error::RuntimeError;

#[derive(Debug, Clone)]
pub(super) struct ActuatorConfig {
    minimum: f64,
    maximum: f64,
    rate_up_per_s: f64,
    rate_down_per_s: f64,
    deadband: f64,
    lag_s: f64,
    initial_applied: f64,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ActuatorSection {
    minimum: Option<f64>,
    maximum: Option<f64>,
    rate_up_per_s: Option<f64>,
    rate_down_per_s: Option<f64>,
    deadband: Option<f64>,
    lag_s: Option<f64>,
    initial_applied: Option<f64>,
}

impl ActuatorSection {
    pub(super) fn into_config(
        self,
        default_initial_applied: f64,
    ) -> Result<ActuatorConfig, RuntimeError> {
        let minimum = finite("plants.actuator.minimum", self.minimum.unwrap_or(f64::MIN))?;
        let maximum = finite("plants.actuator.maximum", self.maximum.unwrap_or(f64::MAX))?;
        let rate_up_per_s = nonnegative(
            "plants.actuator.rate_up_per_s",
            self.rate_up_per_s.unwrap_or(f64::MAX),
        )?;
        let rate_down_per_s = nonnegative(
            "plants.actuator.rate_down_per_s",
            self.rate_down_per_s.unwrap_or(f64::MAX),
        )?;
        let deadband = nonnegative("plants.actuator.deadband", self.deadband.unwrap_or(0.0))?;
        let lag_s = nonnegative("plants.actuator.lag_s", self.lag_s.unwrap_or(0.0))?;
        let initial_applied = finite(
            "plants.actuator.initial_applied",
            self.initial_applied.unwrap_or(default_initial_applied),
        )?;
        if minimum > maximum {
            return invalid("plants.actuator.minimum must not exceed maximum");
        }
        if !(minimum..=maximum).contains(&initial_applied) {
            return invalid("plants.actuator.initial_applied must be within saturation limits");
        }
        Ok(ActuatorConfig {
            minimum,
            maximum,
            rate_up_per_s,
            rate_down_per_s,
            deadband,
            lag_s,
            initial_applied,
        })
    }
}

#[derive(Debug, Clone)]
pub(super) struct ActuatorState {
    config: ActuatorConfig,
    accepted_command: f64,
    applied: f64,
    stuck: bool,
    saturation_active: bool,
    slew_limit_active: bool,
    deadband_active: bool,
}

impl ActuatorState {
    pub(super) fn new(config: ActuatorConfig) -> Self {
        let initial = config.initial_applied;
        Self {
            config,
            accepted_command: initial,
            applied: initial,
            stuck: false,
            saturation_active: false,
            slew_limit_active: false,
            deadband_active: false,
        }
    }

    pub(super) fn initial_applied(&self) -> f64 {
        self.applied
    }

    pub(super) fn advance(&mut self, command: f64, step_s: f64) -> f64 {
        if self.stuck {
            return self.applied;
        }
        let command_delta = (command - self.accepted_command).abs();
        self.deadband_active = command_delta > 0.0 && command_delta <= self.config.deadband;
        if command_delta > self.config.deadband {
            self.accepted_command = command;
        }

        self.saturation_active = self.accepted_command < self.config.minimum
            || self.accepted_command > self.config.maximum;
        let saturated = self
            .accepted_command
            .clamp(self.config.minimum, self.config.maximum);
        let change = saturated - self.applied;
        self.slew_limit_active = if change >= 0.0 {
            change > self.config.rate_up_per_s * step_s
        } else {
            -change > self.config.rate_down_per_s * step_s
        };
        let limited_change = if change >= 0.0 {
            change.min(self.config.rate_up_per_s * step_s)
        } else {
            change.max(-self.config.rate_down_per_s * step_s)
        };
        let slew_limited = self.applied + limited_change;

        // Exact lag discretisation avoids a step-size-dependent stability limit.
        if self.config.lag_s == 0.0 {
            self.applied = slew_limited;
        } else {
            let response = 1.0 - (-step_s / self.config.lag_s).exp();
            self.applied += response * (slew_limited - self.applied);
        }
        self.applied
    }

    pub(super) fn stick(&mut self) {
        self.stuck = true;
    }

    pub(super) fn release(&mut self) {
        self.stuck = false;
    }

    pub(super) fn status(&self) -> super::super::ActuatorStatus {
        super::super::ActuatorStatus {
            applied: self.applied,
            saturation_active: self.saturation_active,
            slew_limit_active: self.slew_limit_active,
            deadband_active: self.deadband_active,
            stuck: self.stuck,
        }
    }
}

fn finite(name: &str, value: f64) -> Result<f64, RuntimeError> {
    if !value.is_finite() {
        return invalid(format!("{name} must be finite"));
    }
    Ok(value)
}

fn nonnegative(name: &str, value: f64) -> Result<f64, RuntimeError> {
    let value = finite(name, value)?;
    if value < 0.0 {
        return invalid(format!("{name} must be >= 0"));
    }
    Ok(value)
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}
