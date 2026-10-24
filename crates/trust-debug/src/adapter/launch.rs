//! Launch argument helpers.
//! - launch_program_path: extract program path
//! - launch_stop_on_entry: stop-on-entry flag
//! - source_options_from_launch, source_options_from_attach: derive source filtering options

use std::collections::BTreeMap;

use serde_json::Value;

use crate::protocol::{AttachArguments, LaunchArguments};
use crate::session::SourceOptionsUpdate;

pub(super) fn launch_program_path(args: &LaunchArguments) -> Option<String> {
    args.additional
        .get("program")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

/// The execution backend the launch asks for: `vm` keeps the bytecode engine, as
/// `trust-runtime run --execution-backend vm` does (specification 44 NX-16).
pub(super) fn launch_execution_backend(args: &LaunchArguments) -> Option<String> {
    args.additional
        .get("executionBackend")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub(super) fn launch_stop_on_entry(args: &LaunchArguments) -> bool {
    args.additional
        .get("stopOnEntry")
        .and_then(|value| value.as_bool())
        .unwrap_or(false)
}

pub(super) fn source_options_from_launch(args: &LaunchArguments) -> SourceOptionsUpdate {
    source_options(&args.additional)
}

/// The source options of an attach: the project with whose types the debug expressions of a
/// runtime that runs a compiled program are compiled (specification 11 §6.13.2).
pub(super) fn source_options_from_attach(args: &AttachArguments) -> SourceOptionsUpdate {
    source_options(&args.additional)
}

fn source_options(additional: &BTreeMap<String, Value>) -> SourceOptionsUpdate {
    SourceOptionsUpdate {
        root: runtime_root(additional),
        include_globs: string_list(additional, "runtimeIncludeGlobs"),
        exclude_globs: string_list(additional, "runtimeExcludeGlobs"),
        ignore_pragmas: string_list(additional, "runtimeIgnorePragmas"),
    }
}

pub(super) fn launch_control_endpoint(args: &LaunchArguments) -> Option<String> {
    args.additional
        .get("controlEndpoint")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub(super) fn launch_control_auth_token(args: &LaunchArguments) -> Option<String> {
    args.additional
        .get("controlAuthToken")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
}

pub(super) fn launch_runtime_root(args: &LaunchArguments) -> Option<String> {
    runtime_root(&args.additional)
}

fn runtime_root(additional: &BTreeMap<String, Value>) -> Option<String> {
    additional
        .get("runtimeRoot")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            additional
                .get("cwd")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn string_list(additional: &BTreeMap<String, Value>, key: &str) -> Option<Vec<String>> {
    additional.get(key).and_then(parse_string_list)
}

fn parse_string_list(value: &Value) -> Option<Vec<String>> {
    let Value::Array(items) = value else {
        return None;
    };
    let mut result = Vec::new();
    for item in items {
        let text = item.as_str()?;
        result.push(text.to_string());
    }
    Some(result)
}
