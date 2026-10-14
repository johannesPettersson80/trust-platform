//! The plant that every case of one Validate runs on, decided from the target-response probe (specification 35, B5.3): the
//! target-response run of the current settings on the configured plant. When its first measured output lies outside the settling
//! band around the selected model's operating point output, the configured plant cannot show the response from that operating
//! point, and the model replaces it for every case. The rule is pure: the engine runs the probe and passes its answer here.

use crate::commissioning::initial_state::operating_output;
use crate::loop_definition::LoopDefinition;
use crate::model::GeneralModel;
use crate::signals::Quality;
use crate::tuning::texts::significant;
use crate::validation::isolated_run_protocol::IsolatedRunStatus;

/// The first measured output of a run: the measured output's value at the run's first sample. A bad or missing value is no
/// measurement (A4.2), so it gives none, and so does a run without a record.
#[must_use]
pub fn first_measured_output(probe: &IsolatedRunStatus, symbol: &str) -> Option<f64> {
    let sample = probe.run.as_ref()?.samples.first()?;
    let value = sample.values.get(symbol)?;
    match value.quality {
        Quality::Good | Quality::Forced => value.value.filter(|measured| measured.is_finite()),
        Quality::Bad | Quality::Missing => None,
    }
}

/// Whether a measured output lies outside the settling band around the operating point: its distance from the point is more than
/// the band (B5.3). A distance equal to the band is inside.
#[must_use]
pub fn outside_settling_band(measured: f64, operating_output: f64, settling_band: f64) -> bool {
    (measured - operating_output).abs() > settling_band
}

/// The reason the model replaces the configured plant for every case, or `None` when the configured plant runs them (B5.3). The
/// probe is the target-response run of the current settings on the configured plant, and its first measured output decides: outside
/// the settling band around the model's operating point output, the reason names the values involved. Without a measured first
/// output the configured plant keeps the cases, and the runs judge their own evidence.
#[must_use]
pub fn model_replacement(
    probe: &IsolatedRunStatus,
    loop_definition: &LoopDefinition,
    model: &GeneralModel,
    settling_band: f64,
) -> Option<String> {
    let process = &loop_definition.signals.process_variable;
    let measured = first_measured_output(probe, &process.symbol)?;
    let origin = operating_output(model, 0);
    outside_settling_band(measured, origin, settling_band)
        .then(|| replacement_reason(measured, origin, settling_band, &process.unit))
}

/// The reason in words (A11.1): the first measured output, the settling band and the operating point, each with the unit.
// The sentence belongs with the other sentences of Validate in `validation::texts`; it stays here until that module is free.
fn replacement_reason(measured: f64, origin: f64, settling_band: f64, unit: &str) -> String {
    format!(
        "The target-response run of the current settings starts at {}, outside the settling band of {} around the model's operating point {}. The configured plant cannot show the response from there, so every case runs on the model.",
        quantity(measured, unit),
        quantity(settling_band, unit),
        quantity(origin, unit),
    )
}

/// A value with its unit, to four significant digits, or the value alone for a dimensionless signal.
fn quantity(value: f64, unit: &str) -> String {
    let value = significant(value, 4);
    if unit.is_empty() {
        value
    } else {
        format!("{value} {unit}")
    }
}

#[cfg(test)]
#[path = "plant_choice_tests.rs"]
mod tests;
