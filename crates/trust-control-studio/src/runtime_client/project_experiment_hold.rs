//! The applied hold of a project step test (`projectExperimentHold.ts`; specification 35, A3.2 and A10.2). A project step
//! test whose hold is a declared experiment parameter records its baseline, every scan of the applied hold and one
//! restoration sample. The duration rule is trust-control's (`project_step_recording_ms`); this module reads the applied
//! hold from the runtime.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::loop_definition::LoopDefinition;

use super::RuntimeClient;

pub use trust_control::experiment::project_step_recording_ms;

/// The values of the loop's parameters as the runtime reports them (`values` of `control.parameters.status`, E7.2).
/// The rest of that answer belongs to the parameter transactions, which are not ported yet (their stage is V4).
#[derive(Deserialize)]
struct ParameterValues {
    values: BTreeMap<String, f64>,
}

/// The applied value of the loop's declared hold parameter, read from the runtime's parameter status (E7.2). A loop
/// without a hold parameter yields `None`, and then nothing is connected.
///
/// # Errors
/// The runtime cannot be reached or refuses, its answer is out of protocol, or it does not report the hold as a finite
/// number.
pub fn applied_project_hold_seconds(
    loop_definition: &LoopDefinition,
    connect: impl FnOnce() -> Result<RuntimeClient, Failure>,
) -> Result<Option<f64>, Failure> {
    let Some(symbol) = loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.hold_parameter.as_deref())
    else {
        return Ok(None);
    };
    let mut client = connect()?;
    let answer = client.call(
        "control.parameters.status",
        Some(json!({ "binding": loop_definition.parameter_binding })),
    )?;
    let parameters: ParameterValues = trust_control::json::decode_value(answer)
        .map_err(|_| Failure::invalid_response("control.parameters.status"))?;
    match parameters.values.get(symbol) {
        Some(value) if value.is_finite() => Ok(Some(*value)),
        _ => Err(Failure::new(
            &format!(
                "The runtime did not report {symbol}; inspect and apply the experiment settings first."
            ),
            Cause::Runtime,
            NextAction::None,
        )),
    }
}

#[cfg(test)]
#[path = "project_experiment_hold_tests.rs"]
mod tests;
