//! ST test runner command and agent-facing JSON helpers.

#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt::Write;
use std::path::{Path, PathBuf};
use std::time::Instant;

use anyhow::Context;
use serde_json::{json, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{collect_project_source_files, resolve_sources_root};
use trust_runtime::error::RuntimeError;
use trust_runtime::test_discovery::{DiscoveredTest, TestKind};

use crate::cli::TestOutput;
use crate::style;

include!("test_cmd/models.rs");
include!("test_cmd/command.rs");
include!("test_cmd/output.rs");
include!("test_cmd/discovery.rs");
include!("test_cmd/native.rs");

#[cfg(test)]
#[path = "test_cmd/tests.rs"]
mod tests;

pub(crate) fn run_test_json(
    project: Option<PathBuf>,
    filter: Option<String>,
    list: bool,
    timeout: u64,
) -> anyhow::Result<JsonValue> {
    let project_root = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    let sources_root = resolve_sources_root(&project_root, None)?;

    let sources = load_sources(&sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let discovered_tests = discover_tests(&sources);
    let mut tests = discovered_tests.clone();
    let discovered_total = tests.len();
    if let Some(filter) = filter.as_deref() {
        let needle = filter.to_ascii_lowercase();
        tests.retain(|case| case.name.as_str().to_ascii_lowercase().contains(&needle));
    }

    if list {
        return Ok(json!({
            "version": 1,
            "project": project_root.display().to_string(),
            "mode": "list",
            "discovered_total": discovered_total,
            "filter": filter,
            "tests": tests.iter().map(|case| json!({
                "name": case.name.as_str(),
                "kind": case.kind.label(),
                "file": case.file.display().to_string(),
                "line": case.line,
                "source": case.source_line.as_deref(),
            })).collect::<Vec<_>>(),
        }));
    }

    if tests.is_empty() && discovered_total > 0 {
        return Ok(json!({
            "version": 1,
            "project": project_root.display().to_string(),
            "summary": {
                "total": 0,
                "passed": 0,
                "failed": 0,
                "errors": 0,
                "duration_ms": 0,
                "discovered_total": discovered_total,
            },
            "tests": [],
        }));
    }

    if tests.is_empty() {
        // A project without tests still compiles, so that its errors fail the run (specification
        // 22).
        check_project(&project_root)?;
        return Ok(json!({
            "version": 1,
            "project": project_root.display().to_string(),
            "summary": {
                "total": 0,
                "passed": 0,
                "failed": 0,
                "errors": 0,
                "duration_ms": 0,
                "discovered_total": discovered_total,
            },
            "tests": [],
        }));
    }

    let total_started = Instant::now();
    // The image holds every discovered test program, the filtered-out ones included, and runs
    // only the selected ones, as `run_test` does (specification 22).
    let results = execute_native_tests(&project_root, &discovered_tests, &tests, timeout)?;
    let total_duration_ms = elapsed_ms(total_started.elapsed());
    let summary = summarize_results(&results);
    let rendered = render_json_output(&project_root, &results, summary, total_duration_ms)?;
    let mut payload: JsonValue =
        serde_json::from_str(&rendered).context("parse rendered test json payload")?;
    payload["summary"]["discovered_total"] = json!(discovered_total);
    Ok(payload)
}
