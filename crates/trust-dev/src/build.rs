//! Agent-facing bundle build helpers.

use std::path::{Path, PathBuf};

use serde_json::{json, Value as JsonValue};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{build_program_stbc, BundleBuildReport};

pub(crate) fn build_json_payload(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
) -> anyhow::Result<JsonValue> {
    let bundle_root = match bundle {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let report = build_program_stbc(&bundle_root, sources.as_deref())?;
    Ok(build_payload_from_report(&bundle_root, report))
}

fn build_payload_from_report(bundle_root: &Path, report: BundleBuildReport) -> JsonValue {
    json!({
        "version": 1,
        "command": "build",
        "status": "ok",
        "project": bundle_root.display().to_string(),
        "program": report.program_path.display().to_string(),
        "source_count": report.sources.len(),
        "sources": report.sources.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "dependency_roots": report.dependency_roots.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "resolved_dependencies": report.resolved_dependencies,
    })
}
