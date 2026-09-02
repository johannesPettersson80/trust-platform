//! Enforced operating-region bounds for local plant models.

use serde::Deserialize;

use crate::error::RuntimeError;

#[derive(Debug, Clone, Default)]
pub(super) struct ValidityConfig {
    input: Option<Bounds>,
    state: Option<Bounds>,
    output: Option<Bounds>,
}

#[derive(Debug, Clone)]
struct Bounds {
    minimum: Vec<f64>,
    maximum: Vec<f64>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ValiditySection {
    input_minimum: Option<Vec<f64>>,
    input_maximum: Option<Vec<f64>>,
    state_minimum: Option<Vec<f64>>,
    state_maximum: Option<Vec<f64>>,
    output_minimum: Option<Vec<f64>>,
    output_maximum: Option<Vec<f64>>,
    out_of_range: Option<String>,
}

impl ValiditySection {
    pub(super) fn into_config(
        self,
        input_count: usize,
        state_count: usize,
        output_count: usize,
    ) -> Result<ValidityConfig, RuntimeError> {
        if self
            .out_of_range
            .as_deref()
            .is_some_and(|value| value != "fault")
        {
            return invalid("plants.validity.out_of_range supports only 'fault'");
        }
        Ok(ValidityConfig {
            input: Bounds::from_pair("input", self.input_minimum, self.input_maximum, input_count)?,
            state: Bounds::from_pair("state", self.state_minimum, self.state_maximum, state_count)?,
            output: Bounds::from_pair(
                "output",
                self.output_minimum,
                self.output_maximum,
                output_count,
            )?,
        })
    }
}

impl ValidityConfig {
    pub(super) fn validate_initial(
        &self,
        input: &[f64],
        state: &[f64],
    ) -> Result<(), RuntimeError> {
        self.check_configured("input", &self.input, input)?;
        self.check_configured("state", &self.state, state)
    }

    pub(super) fn check_input(&self, plant_id: &str, values: &[f64]) -> Result<(), RuntimeError> {
        self.check_runtime(plant_id, "input", &self.input, values)
    }

    pub(super) fn check_state(&self, plant_id: &str, values: &[f64]) -> Result<(), RuntimeError> {
        self.check_runtime(plant_id, "state", &self.state, values)
    }

    pub(super) fn check_output(&self, plant_id: &str, values: &[f64]) -> Result<(), RuntimeError> {
        self.check_runtime(plant_id, "output", &self.output, values)
    }

    fn check_configured(
        &self,
        name: &str,
        bounds: &Option<Bounds>,
        values: &[f64],
    ) -> Result<(), RuntimeError> {
        let Some((channel, value, minimum, maximum)) = outside(bounds, values) else {
            return Ok(());
        };
        invalid(format!(
            "plants initial {name} channel {channel} value {value} is outside [{minimum}, {maximum}]"
        ))
    }

    fn check_runtime(
        &self,
        plant_id: &str,
        name: &str,
        bounds: &Option<Bounds>,
        values: &[f64],
    ) -> Result<(), RuntimeError> {
        let Some((channel, value, minimum, maximum)) = outside(bounds, values) else {
            return Ok(());
        };
        Err(RuntimeError::SimulationFault(
            format!(
                "plant '{plant_id}' validity {name} channel {channel} value {value} outside [{minimum}, {maximum}]"
            )
            .into(),
        ))
    }
}

impl Bounds {
    fn from_pair(
        name: &str,
        minimum: Option<Vec<f64>>,
        maximum: Option<Vec<f64>>,
        expected: usize,
    ) -> Result<Option<Self>, RuntimeError> {
        let (minimum, maximum) = match (minimum, maximum) {
            (None, None) => return Ok(None),
            (Some(minimum), Some(maximum)) => (minimum, maximum),
            _ => {
                return invalid(format!(
                    "plants.validity.{name}_minimum and {name}_maximum must be declared together"
                ))
            }
        };
        if minimum.len() != expected || maximum.len() != expected {
            return invalid(format!(
                "plants.validity {name} bounds require exactly {expected} values"
            ));
        }
        if minimum
            .iter()
            .zip(&maximum)
            .any(|(low, high)| !low.is_finite() || !high.is_finite() || low > high)
        {
            return invalid(format!(
                "plants.validity {name} bounds must be finite and ordered"
            ));
        }
        Ok(Some(Self { minimum, maximum }))
    }
}

fn outside(bounds: &Option<Bounds>, values: &[f64]) -> Option<(usize, f64, f64, f64)> {
    let bounds = bounds.as_ref()?;
    values
        .iter()
        .copied()
        .enumerate()
        .find_map(|(channel, value)| {
            let minimum = bounds.minimum[channel];
            let maximum = bounds.maximum[channel];
            (!(minimum..=maximum).contains(&value)).then_some((channel, value, minimum, maximum))
        })
}

fn invalid<T>(message: impl Into<String>) -> Result<T, RuntimeError> {
    Err(RuntimeError::InvalidConfig(message.into().into()))
}
