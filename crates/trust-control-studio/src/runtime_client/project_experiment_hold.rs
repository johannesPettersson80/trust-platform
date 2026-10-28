//! The applied hold and the experiment settings of a project test (`projectExperimentHold.ts`; specification 35, A3.2, A10.2 and C12
//! Hold parameter and Parameter purpose). A project step test whose hold is a declared experiment parameter records its baseline, every
//! scan of the applied hold and one restoration sample. The duration rule and the refusal of a hold that cannot be recorded are
//! trust-control's (`project_step_recording_ms`, `applied_hold_recording_ms`); this module reads the applied hold from the runtime, for
//! the Identify review and start (`identify_action`) and for the connected run (`ConnectedRuntime::applied_hold_seconds`). The review
//! and the start of a project test also read the values of the settings that the loop's parameter tool writes, which the review's
//! identity binds, from the same read of the record ([`applied_project_settings_over`]).

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::json;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::identification::connected_run::runtime_required;
use trust_control::identification::policy::project_scripted;
use trust_control::identification::project_test::parameter_tool;
use trust_control::loop_definition::LoopDefinition;
use trust_control::runtime::RuntimeConnection;

use super::RuntimeClient;

pub use trust_control::experiment::project_step_recording_ms;

/// The values of the loop's parameters as the runtime reports them (`values` of `control.parameters.status`, E7.2).
/// The rest of that answer is the parameter record, which the parameter transactions check.
#[derive(Deserialize)]
struct ParameterValues {
    values: BTreeMap<String, f64>,
}

/// What the review and the start of a project test read from the runtime's parameter record (C12): the hold the runtime applies, when
/// the loop names a hold parameter, and the values of the settings that the loop's parameter tool writes, when the loop has the tool,
/// which the review's identity binds (Parameter purpose: the settings are frozen through review).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AppliedProjectSettings {
    /// The applied value of the hold parameter, in seconds; `None` for a loop that names none.
    pub hold_seconds: Option<f64>,
    /// The values of the record, by symbol; `None` for a loop that is no project test with a parameter tool.
    pub values: Option<BTreeMap<String, f64>>,
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
    let Some(symbol) = hold_symbol(loop_definition) else {
        return Ok(None);
    };
    let values = parameter_values(loop_definition, connect)?;
    applied_hold(&values, symbol).map(Some)
}

/// The applied value of the loop's declared hold parameter over the `connection` a request carries, or `None` for a loop that declares
/// none, which reads nothing (C12, Hold parameter). A request that carries no connection cannot read one, and says that the runtime
/// must be started.
///
/// # Errors
/// There is no connection, or as [`applied_project_hold_seconds`].
pub fn applied_project_hold_over(
    loop_definition: &LoopDefinition,
    connection: Option<&RuntimeConnection>,
) -> Result<Option<f64>, Failure> {
    applied_project_hold_seconds(loop_definition, || {
        RuntimeClient::connect(connection.ok_or_else(runtime_required)?)
    })
}

/// The hold and the settings of a project test over the `connection` a request carries (C12): one read of the runtime's parameter
/// record gives the applied hold, when the loop names a hold parameter, and the values of the record, when the loop is a project test
/// with a parameter tool. A loop with neither reads nothing.
///
/// # Errors
/// There is no connection; the runtime cannot be reached or refuses, or its answer is out of protocol; or it does not report the hold
/// as a finite number (as [`applied_project_hold_seconds`]).
pub fn applied_project_settings_over(
    loop_definition: &LoopDefinition,
    connection: Option<&RuntimeConnection>,
) -> Result<AppliedProjectSettings, Failure> {
    let hold = hold_symbol(loop_definition);
    let binds_settings =
        project_scripted(loop_definition).is_some() && parameter_tool(loop_definition).is_some();
    if hold.is_none() && !binds_settings {
        return Ok(AppliedProjectSettings::default());
    }
    let values = parameter_values(loop_definition, || {
        RuntimeClient::connect(connection.ok_or_else(runtime_required)?)
    })?;
    let hold_seconds = hold
        .map(|symbol| applied_hold(&values, symbol))
        .transpose()?;
    Ok(AppliedProjectSettings {
        hold_seconds,
        values: binds_settings.then_some(values),
    })
}

/// The symbol of the loop's declared hold parameter, if it names one (C12).
fn hold_symbol(loop_definition: &LoopDefinition) -> Option<&str> {
    loop_definition
        .experiment
        .as_ref()
        .and_then(|experiment| experiment.hold_parameter.as_deref())
}

/// The values of the loop's parameter record, as the runtime reports them (E7.2).
fn parameter_values(
    loop_definition: &LoopDefinition,
    connect: impl FnOnce() -> Result<RuntimeClient, Failure>,
) -> Result<BTreeMap<String, f64>, Failure> {
    let mut client = connect()?;
    let answer = client.call(
        "control.parameters.status",
        Some(json!({ "binding": loop_definition.parameter_binding })),
    )?;
    let parameters: ParameterValues = trust_control::json::decode_value(answer)
        .map_err(|_| Failure::invalid_response("control.parameters.status"))?;
    Ok(parameters.values)
}

/// The applied hold among `values`, which must report it as a finite number.
fn applied_hold(values: &BTreeMap<String, f64>, symbol: &str) -> Result<f64, Failure> {
    match values.get(symbol) {
        Some(value) if value.is_finite() => Ok(*value),
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
