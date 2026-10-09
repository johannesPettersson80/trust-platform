//! The loop's mode binding (Specification 35, A6.1 and A6.5): the mode signal and the value that means Manual. An experiment
//! that writes the plant input or the manual output runs only in Manual; a setpoint or load experiment runs only in Auto.

use serde::Deserialize;
use serde_json::Value as JsonValue;

use super::values::numeric_value;
use crate::debug::{resolve_control_parameter_path, DebugControl};
use crate::memory::VariableStorage;
use crate::value::{Value, ValueRef};

type Refusal = (String, &'static str);

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct ModeBindingParams {
    symbol: String,
    manual_value: JsonValue,
}

#[derive(Clone, Debug)]
pub(super) struct ModeBinding {
    path: String,
    reference: ValueRef,
    /// The value that means Manual, as a number: a bool is 1 or 0, an enumeration its numeric value.
    manual: f64,
    /// Whether the experiment needs Manual (true) or Auto (false).
    manual_required: bool,
}

impl ModeBinding {
    /// Resolves the binding at start. The symbol must exist and hold a bool, a number or an enumeration, and the loop must
    /// already be in the mode the experiment needs.
    pub(super) fn start(
        params: ModeBindingParams,
        manual_required: bool,
        debug: &DebugControl,
    ) -> Result<Self, Refusal> {
        let manual = match &params.manual_value {
            JsonValue::Bool(value) => f64::from(u8::from(*value)),
            JsonValue::Number(number) => number
                .as_f64()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    invalid_mode(&params.symbol, "manual_value must be a finite number")
                })?,
            _ => {
                return Err(invalid_mode(
                    &params.symbol,
                    "manual_value must be a boolean or a number",
                ))
            }
        };
        let resolved = debug
            .with_snapshot(|captured| {
                resolve_control_parameter_path(&captured.storage, &params.symbol)
            })
            .ok_or_else(|| invalid_mode(&params.symbol, "no runtime scan has been captured yet"))?
            .map_err(|reason| invalid_mode(&params.symbol, reason.as_str()))?;
        let mode = mode_number(&resolved.value).ok_or_else(|| {
            invalid_mode(
                &params.symbol,
                "the mode signal is not a boolean, a number or an enumeration",
            )
        })?;
        if (mode == manual) != manual_required {
            let wanted = if manual_required { "Manual" } else { "Auto" };
            return Err((
                format!(
                    "the loop must be in {wanted} mode before this experiment starts; {} shows another mode",
                    params.symbol
                ),
                "controller_mode_blocked",
            ));
        }
        Ok(Self {
            path: params.symbol,
            reference: resolved.reference,
            manual,
            manual_required,
        })
    }

    /// Whether the loop holds the mode the experiment needs in this scan's storage. `Err` names why the mode cannot be read.
    pub(super) fn holds_required_mode(
        &self,
        storage: &VariableStorage,
    ) -> Result<bool, &'static str> {
        let mode = resolve_control_parameter_path(storage, &self.path)
            .ok()
            .filter(|resolved| resolved.reference == self.reference)
            .and_then(|resolved| mode_number(&resolved.value))
            .ok_or("bad_runtime_data")?;
        Ok((mode == self.manual) == self.manual_required)
    }
}

/// The mode signal as a number: a bool is 1 or 0, and an enumeration compares by its numeric value.
fn mode_number(value: &Value) -> Option<f64> {
    match value {
        Value::Enum(enumeration) => Some(enumeration.numeric_value() as f64),
        other => numeric_value(other),
    }
}

fn invalid_mode(symbol: &str, reason: &str) -> Refusal {
    (
        format!("mode binding {symbol}: {reason}"),
        "invalid_experiment",
    )
}
