//! State-space matrix dimensions and continuous input-delay configuration.
use super::{channel_setting, invalid, PlantModelConfig, PlantParametersSection, RuntimeError};

pub(super) fn input_delays(
    seconds: Option<Vec<f64>>,
    inputs: usize,
) -> Result<Vec<i64>, RuntimeError> {
    channel_setting("plants.parameters.input_delays_s", seconds, inputs, 0.0)?
        .into_iter()
        .map(|seconds| {
            if seconds < 0.0 {
                return invalid("plants.parameters.input_delays_s must be nonnegative");
            }
            let nanos = (seconds * 1_000_000_000.0).round();
            if !nanos.is_finite() || nanos >= i64::MAX as f64 {
                return invalid(
                    "plants.parameters.input_delays_s exceeds the nanosecond time range",
                );
            }
            Ok(nanos as i64)
        })
        .collect()
}

pub(super) fn model(
    parameters: &mut PlantParametersSection,
    command_count: usize,
    feedback_count: usize,
) -> Result<PlantModelConfig, RuntimeError> {
    let a = parameters
        .a
        .take()
        .ok_or_else(|| RuntimeError::InvalidConfig("state_space requires matrix a".into()))?;
    let b = parameters
        .b
        .take()
        .ok_or_else(|| RuntimeError::InvalidConfig("state_space requires matrix b".into()))?;
    let c = parameters
        .c
        .take()
        .ok_or_else(|| RuntimeError::InvalidConfig("state_space requires matrix c".into()))?;
    let d = parameters
        .d
        .take()
        .ok_or_else(|| RuntimeError::InvalidConfig("state_space requires matrix d".into()))?;
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
    Ok(PlantModelConfig::StateSpace { a, b, c, d })
}
