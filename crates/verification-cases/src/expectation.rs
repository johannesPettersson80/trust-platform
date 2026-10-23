//! Enforcement of a case's declared `expect` block against an observed operation.
//!
//! [`run_case_file`](crate::run_case_file) records before and after state for a
//! whole case but never reads `expect`, so a case that declares a rejection, an
//! atomic refusal or an unchanged target passes whatever the runner did. A runner
//! calls [`check_case_expectation`] once per operation under test, passing the
//! outcome it observed and the state on both sides of that operation.
//!
//! The vocabulary is closed. Any other `expect` key, any `delta` key, and any
//! value outside the lists below is an error, so a declaration that nothing
//! evaluates cannot pass silently.
//!
//! - `outcome`: compared with the observed outcome, one of [`OUTCOMES`].
//! - `no_partial_apply = true`: for an observed outcome other than
//!   `accept_value`, the target, siblings, retain and process-image components
//!   are unchanged. For `accept_value` the accepted values are checked by the
//!   runner's trace comparison.
//! - `fault_surface`: compared with the observed fault surface.
//! - `oracle_ref`: a specification reference; recorded, not evaluated.
//! - `delta.target`, `delta.siblings`, `delta.retain`, `delta.process_image`:
//!   `unchanged` compares the state before and after the operation.
//!   `set_to_expected` and `expected_delta` name values that the runner's trace
//!   comparison checks, so they are accepted here without evaluation.
//!   `not_applicable` is skipped.
//! - `delta.diagnostics`: `none` requires no diagnostic after the operation,
//!   `stable_error` requires at least one, `not_applicable` is skipped.
//! - `delta.status`: `unchanged` compares the status before and after,
//!   `fault_visible` and `not_safe_reported` require that status after the
//!   operation, `not_applicable` is skipped.

use std::collections::BTreeMap;

use crate::{CaseRecord, StateSnapshot};

/// Outcomes a runner may observe and a case may declare.
pub const OUTCOMES: &[&str] = &[
    "accept_value",
    "reject",
    "fault_visible",
    "not_safe_reported",
];

const EXPECT_KEYS: &[&str] = &[
    "outcome",
    "no_partial_apply",
    "oracle_ref",
    "fault_surface",
    "delta",
];
const DELTA_KEYS: &[&str] = &[
    "target",
    "siblings",
    "retain",
    "process_image",
    "diagnostics",
    "status",
];
const COMPONENT_KEYS: &[&str] = &["target", "siblings", "retain", "process_image"];

/// One operation under test, as the runner observed it.
#[derive(Clone, Copy, Debug)]
pub struct ObservedOperation<'a> {
    /// The observed outcome, one of [`OUTCOMES`].
    pub outcome: &'a str,
    /// The observed fault surface, when the operation reported one.
    pub fault_surface: Option<&'a str>,
    /// The runtime status before the operation, when the runner observes it.
    pub status_before: Option<&'a str>,
    /// The runtime status after the operation, when the runner observes it.
    pub status_after: Option<&'a str>,
    /// The state before the operation.
    pub before: &'a StateSnapshot,
    /// The state after the operation.
    pub after: &'a StateSnapshot,
}

/// Checks the `expect` block of `case` against `observed`.
///
/// Returns the declared expectations that do not hold. The list is empty when
/// the operation meets every expectation that the case declares.
///
/// # Errors
///
/// Returns an error when the case has no `expect` block, when `expect` or
/// `expect.delta` contains a key or value outside the vocabulary, or when the
/// observed outcome is not in [`OUTCOMES`].
pub fn check_case_expectation(
    case: &CaseRecord,
    observed: &ObservedOperation<'_>,
) -> Result<Vec<String>, String> {
    let expect = case
        .expect
        .as_ref()
        .ok_or_else(|| format!("case {} declares no expect block", case.id))?;
    check_expectation(expect, observed)
}

/// Checks a declared `expect` value against `observed`.
///
/// # Errors
///
/// Returns the same errors as [`check_case_expectation`].
pub fn check_expectation(
    expect: &toml::Value,
    observed: &ObservedOperation<'_>,
) -> Result<Vec<String>, String> {
    let table = expect
        .as_table()
        .ok_or_else(|| "expect must be a table".to_string())?;
    if let Some(key) = table
        .keys()
        .find(|key| !EXPECT_KEYS.contains(&key.as_str()))
    {
        return Err(format!("unknown expect key {key:?}"));
    }
    if !OUTCOMES.contains(&observed.outcome) {
        return Err(format!(
            "observed outcome {:?} is not in the vocabulary",
            observed.outcome
        ));
    }

    let mut mismatches = Vec::new();
    if let Some(value) = table.get("outcome") {
        let declared = text(value, "outcome")?;
        if !OUTCOMES.contains(&declared) {
            return Err(format!("unknown expect.outcome {declared:?}"));
        }
        if declared != observed.outcome {
            mismatches.push(format!(
                "outcome expected {declared:?}, observed {:?}",
                observed.outcome
            ));
        }
    }
    if let Some(value) = table.get("no_partial_apply") {
        let flag = value
            .as_bool()
            .ok_or_else(|| "expect.no_partial_apply must be a boolean".to_string())?;
        if flag && observed.outcome != "accept_value" {
            for component in COMPONENT_KEYS {
                if component_changed(component, observed.before, observed.after) {
                    mismatches.push(format!(
                        "no_partial_apply: {component} changed by a {} operation",
                        observed.outcome
                    ));
                }
            }
        }
    }
    if let Some(value) = table.get("fault_surface") {
        let declared = text(value, "fault_surface")?;
        if observed.fault_surface != Some(declared) {
            mismatches.push(format!(
                "fault_surface expected {declared:?}, observed {:?}",
                observed.fault_surface
            ));
        }
    }
    if let Some(value) = table.get("oracle_ref") {
        text(value, "oracle_ref")?;
    }
    if let Some(value) = table.get("delta") {
        let delta = value
            .as_table()
            .ok_or_else(|| "expect.delta must be a table".to_string())?;
        for (key, value) in delta {
            let declared = text(value, key)?;
            check_delta(key, declared, observed, &mut mismatches)?;
        }
    }
    Ok(mismatches)
}

/// Rejects a trace `expected` field that the runner does not read.
///
/// A runner passes the fields it compares. A field outside that list would
/// otherwise be ignored, so the case could declare a value that nothing checks.
///
/// # Errors
///
/// Returns an error that names the first field outside `understood`.
pub fn check_trace_fields(
    expected: &BTreeMap<String, toml::Value>,
    understood: &[&str],
) -> Result<(), String> {
    match expected
        .keys()
        .find(|field| !understood.contains(&field.as_str()))
    {
        Some(field) => Err(format!("unknown trace expected field {field:?}")),
        None => Ok(()),
    }
}

fn check_delta(
    key: &str,
    declared: &str,
    observed: &ObservedOperation<'_>,
    mismatches: &mut Vec<String>,
) -> Result<(), String> {
    if !DELTA_KEYS.contains(&key) {
        return Err(format!("unknown expect.delta key {key:?}"));
    }
    match key {
        "target" | "siblings" | "retain" | "process_image" => match declared {
            "unchanged" => {
                if component_changed(key, observed.before, observed.after) {
                    mismatches.push(format!(
                        "delta.{key} expected unchanged, but the operation changed it"
                    ));
                }
            }
            "set_to_expected" | "expected_delta" | "not_applicable" => {}
            other => return Err(format!("unknown expect.delta.{key} value {other:?}")),
        },
        "diagnostics" => match declared {
            "none" => {
                if !observed.after.diagnostics.is_empty() {
                    mismatches.push(format!(
                        "delta.diagnostics expected none, observed {:?}",
                        observed.after.diagnostics
                    ));
                }
            }
            "stable_error" => {
                if observed.after.diagnostics.is_empty() {
                    mismatches
                        .push("delta.diagnostics expected a stable error, observed none".into());
                }
            }
            "not_applicable" => {}
            other => return Err(format!("unknown expect.delta.diagnostics value {other:?}")),
        },
        _ => match declared {
            "unchanged" => {
                if observed.status_before != observed.status_after {
                    mismatches.push(format!(
                        "delta.status expected unchanged, observed {:?} then {:?}",
                        observed.status_before, observed.status_after
                    ));
                }
            }
            "fault_visible" | "not_safe_reported" => {
                if observed.status_after != Some(declared) {
                    mismatches.push(format!(
                        "delta.status expected {declared:?}, observed {:?}",
                        observed.status_after
                    ));
                }
            }
            "not_applicable" => {}
            other => return Err(format!("unknown expect.delta.status value {other:?}")),
        },
    }
    Ok(())
}

fn component_changed(component: &str, before: &StateSnapshot, after: &StateSnapshot) -> bool {
    match component {
        "target" => before.target != after.target,
        "siblings" => before.siblings != after.siblings,
        "retain" => before.retain_hash != after.retain_hash,
        "process_image" => before.process_image_hash != after.process_image_hash,
        _ => false,
    }
}

fn text<'a>(value: &'a toml::Value, name: &str) -> Result<&'a str, String> {
    value
        .as_str()
        .ok_or_else(|| format!("expect.{name} must be text"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::{check_expectation, check_trace_fields, ObservedOperation};
    use crate::StateSnapshot;

    fn snapshot(target: i64, diagnostics: &[&str]) -> StateSnapshot {
        StateSnapshot {
            process_image_hash: None,
            retain_hash: None,
            target: Some(serde_json::json!(target)),
            siblings: BTreeMap::new(),
            diagnostics: diagnostics.iter().map(|text| (*text).to_string()).collect(),
        }
    }

    fn expect(text: &str) -> toml::Value {
        toml::from_str::<toml::Table>(text)
            .map(toml::Value::Table)
            .expect("test expect block must parse")
    }

    #[test]
    fn rejection_with_changed_target_is_a_mismatch() {
        let before = snapshot(7, &[]);
        let after = snapshot(100, &["outside declared subrange 0..10"]);
        let observed = ObservedOperation {
            outcome: "reject",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before,
            after: &after,
        };
        let declared = expect(
            r#"outcome = "reject"
no_partial_apply = true
delta = { target = "unchanged", diagnostics = "stable_error" }"#,
        );
        let mismatches = check_expectation(&declared, &observed).expect("keys are known");
        assert_eq!(mismatches.len(), 2, "{mismatches:?}");
        assert!(mismatches[0].contains("no_partial_apply: target"));
        assert!(mismatches[1].contains("delta.target expected unchanged"));
    }

    #[test]
    fn matching_rejection_has_no_mismatch() {
        let before = snapshot(7, &[]);
        let after = snapshot(7, &["outside declared subrange 0..10"]);
        let observed = ObservedOperation {
            outcome: "reject",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before,
            after: &after,
        };
        let declared = expect(
            r#"outcome = "reject"
no_partial_apply = true
delta = { target = "unchanged", diagnostics = "stable_error", status = "not_applicable" }"#,
        );
        let mismatches = check_expectation(&declared, &observed).expect("keys are known");
        assert!(mismatches.is_empty(), "{mismatches:?}");
    }

    #[test]
    fn unknown_expect_key_is_an_error() {
        let before = snapshot(1, &[]);
        let observed = ObservedOperation {
            outcome: "accept_value",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before,
            after: &before,
        };
        let declared = expect(
            r#"outcome = "accept_value"
surprise = true"#,
        );
        let error = check_expectation(&declared, &observed).expect_err("unknown key must fail");
        assert!(error.contains("surprise"), "{error}");
    }

    #[test]
    fn unknown_delta_value_is_an_error() {
        let before = snapshot(1, &[]);
        let observed = ObservedOperation {
            outcome: "accept_value",
            fault_surface: None,
            status_before: None,
            status_after: None,
            before: &before,
            after: &before,
        };
        let declared = expect(r#"delta = { target = "roughly_unchanged" }"#);
        let error = check_expectation(&declared, &observed).expect_err("unknown value must fail");
        assert!(error.contains("roughly_unchanged"), "{error}");
    }

    #[test]
    fn unknown_trace_field_is_an_error() {
        let mut expected = BTreeMap::new();
        expected.insert("count".to_string(), toml::Value::Integer(1));
        expected.insert("cycle_count".to_string(), toml::Value::Integer(1));
        let error = check_trace_fields(&expected, &["count"]).expect_err("unread field must fail");
        assert!(error.contains("cycle_count"), "{error}");
    }
}
