//! Launch argument helpers.
//! - launch_program_path: extract program path
//! - launch_stop_on_entry: stop-on-entry flag
//! - source_options_from_launch: derive source filtering options

use serde_json::Value;

use crate::protocol::LaunchArguments;
use crate::session::SourceOptionsUpdate;

pub(super) fn launch_program_path(args: &LaunchArguments) -> Option<String> {
    args.additional
        .get("program")
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
    SourceOptionsUpdate {
        root: launch_runtime_root(args),
        include_globs: launch_string_list(args, "runtimeIncludeGlobs"),
        exclude_globs: launch_string_list(args, "runtimeExcludeGlobs"),
        ignore_pragmas: launch_string_list(args, "runtimeIgnorePragmas"),
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
    args.additional
        .get("runtimeRoot")
        .and_then(|value| value.as_str())
        .map(|value| value.to_string())
        .or_else(|| {
            args.additional
                .get("cwd")
                .and_then(|value| value.as_str())
                .map(|value| value.to_string())
        })
}

fn launch_string_list(args: &LaunchArguments, key: &str) -> Option<Vec<String>> {
    args.additional.get(key).and_then(parse_string_list)
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
