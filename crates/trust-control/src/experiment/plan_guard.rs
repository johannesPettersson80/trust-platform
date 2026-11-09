//! The measurement guard of a reviewed plan (specification 35, B3.4 step 6; runtime protocol annex E5.3): the loop's
//! measured output and its permitted range. The runtime watches it while the experiment runs and stops the experiment,
//! restoring what it wrote, once the measurement leaves the range.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

use super::PlanSignal;
use crate::experiment::number_text::{js_number, js_to_precision};
use crate::loop_definition::LoopDefinition;

/// The significant digits a stopped guard writes the measurement with (`toPrecision(4)`, as the limits check of B3.4 step
/// 6 writes its findings).
const VALUE_DIGITS: usize = 4;

/// The measured output an experiment must keep within its permitted range (E5.3, `measurement_guard`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PlanMeasurementGuard {
    /// The measured output's symbol.
    pub symbol: String,
    /// Its address, when it has one; the runtime then reads the address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address: Option<String>,
    /// The smallest permitted value.
    pub minimum: f64,
    /// The largest permitted value.
    pub maximum: f64,
    /// The measured output's unit.
    pub unit: String,
}

/// The guard of a plan on `loop_definition` (B3.4 step 6): its measured output, at the address the live sample reads it
/// from, and the permitted range the limits check of the recording uses.
pub(super) fn measurement_guard(
    loop_definition: &LoopDefinition,
    process_variable: &PlanSignal,
) -> PlanMeasurementGuard {
    let output = &loop_definition.signals.process_variable;
    PlanMeasurementGuard {
        symbol: output.symbol.clone(),
        address: process_variable.address.clone(),
        minimum: output.range.minimum,
        maximum: output.range.maximum,
        unit: output.unit.clone(),
    }
}

/// The stop condition of the guard on `loop_definition`, as a plan and its review list it: `LevelPV outside 0 to 100 %`.
#[must_use]
pub fn measurement_stop_condition(loop_definition: &LoopDefinition) -> String {
    format!(
        "{} outside {}",
        loop_definition.signals.process_variable.symbol,
        permitted_range(loop_definition)
    )
}

/// The measured output's permitted range on `loop_definition`, in words: `0 to 100 %`.
#[must_use]
pub fn permitted_range(loop_definition: &LoopDefinition) -> String {
    let output = &loop_definition.signals.process_variable;
    format!(
        "{} to {}",
        js_number(output.range.minimum),
        with_unit(js_number(output.range.maximum), &output.unit)
    )
}

impl PlanMeasurementGuard {
    /// Why the experiment stopped when the guard stopped it, with the value the runtime read (E5.6): `LevelPV reached 104
    /// %, above its permitted maximum of 100 %`. Without the value, the range alone is named.
    #[must_use]
    pub fn stop_reason(&self, value: Option<f64>) -> String {
        let quantity = |number: f64| with_unit(js_number(number), &self.unit);
        match value.filter(|value| value.is_finite()) {
            Some(value) if value > self.maximum => format!(
                "{} reached {}, above its permitted maximum of {}",
                self.symbol,
                quantity(js_to_precision(value, VALUE_DIGITS)),
                quantity(self.maximum)
            ),
            Some(value) if value < self.minimum => format!(
                "{} reached {}, below its permitted minimum of {}",
                self.symbol,
                quantity(js_to_precision(value, VALUE_DIGITS)),
                quantity(self.minimum)
            ),
            _ => format!(
                "{} left its permitted range of {} to {}",
                self.symbol,
                js_number(self.minimum),
                quantity(self.maximum)
            ),
        }
    }
}

/// A number with its unit, `100 %`, or the number alone for a signal without one.
fn with_unit(number: String, unit: &str) -> String {
    if unit.is_empty() {
        number
    } else {
        format!("{number} {unit}")
    }
}
