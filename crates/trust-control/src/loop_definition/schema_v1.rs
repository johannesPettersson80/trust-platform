//! The version 1 loop file (specification 35, B1): the earlier format, which the engine reads by the rules that the `TypeScript`
//! decoder (`decodeControlLoopDefinition`) applies to version 1. A version 1 file declares no key that only version 2 has, and
//! every other key means what it means in version 2. The file is not rewritten: the document is read as the version 2 shape, so
//! the typed loop is the one a version 2 file gives.

use serde_json::Value;

use super::{LoopRejection, LOOP_SCHEMA_VERSION};

/// The version of the earlier loop file format that this module reads.
pub(super) const SCHEMA_VERSION: u32 = 1;

/// The keys a version 1 loop does not declare at its top level: the parameter binding and its purpose, the verification scenarios,
/// the channels, and the declared analysis with its parameter symbols.
const TOP_LEVEL_KEYS: [&str; 6] = [
    "parameterBinding",
    "parameterPurpose",
    "verificationScenarios",
    "channels",
    "analysis",
    "analysisParameterSymbols",
];

/// The keys a version 1 `controller` does not declare: the law, state feedback, an ON/OFF controller, the program's own law and a PID
/// group.
const CONTROLLER_KEYS: [&str; 5] = ["law", "stateFeedback", "onOff", "project", "group"];

/// The keys a version 1 `experiment` does not declare: the abort variable, the baseline and the hold parameter.
const EXPERIMENT_KEYS: [&str; 3] = ["abort", "baselineSeconds", "holdParameter"];

/// Reads a version 1 loop document as the version 2 shape, in memory: refuses the first key that only version 2 declares, and then
/// sets the document's version to `LOOP_SCHEMA_VERSION`.
///
/// # Errors
/// The document declares a key that version 1 does not have. The refusal names the key.
pub(super) fn read_as_current(document: &mut Value) -> Result<(), LoopRejection> {
    if let Some(key) = declared_version_two_key(document) {
        return Err(LoopRejection::Malformed(format!(
            "schema version {SCHEMA_VERSION} does not declare {key}, which needs schema version {LOOP_SCHEMA_VERSION}"
        )));
    }
    if let Some(fields) = document.as_object_mut() {
        fields.insert("schemaVersion".into(), Value::from(LOOP_SCHEMA_VERSION));
    }
    Ok(())
}

/// The first key that only version 2 declares, named by its path. A key that is present is declared even when its value is `null`,
/// as the `TypeScript` decoder reads it (`!== undefined`).
fn declared_version_two_key(document: &Value) -> Option<String> {
    first_declared(document, &TOP_LEVEL_KEYS, "")
        .or_else(|| {
            document
                .get("controller")
                .and_then(|controller| first_declared(controller, &CONTROLLER_KEYS, "controller."))
        })
        .or_else(|| {
            document
                .get("experiment")
                .and_then(|experiment| first_declared(experiment, &EXPERIMENT_KEYS, "experiment."))
        })
}

/// The first of `keys` that `object` holds, with `prefix` in front of its name.
fn first_declared(object: &Value, keys: &[&str], prefix: &str) -> Option<String> {
    keys.iter()
        .find(|key| object.get(**key).is_some())
        .map(|key| format!("{prefix}{key}"))
}

#[cfg(test)]
#[path = "schema_v1_tests.rs"]
mod tests;
