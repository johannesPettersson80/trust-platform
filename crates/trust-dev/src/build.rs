//! Agent-facing bundle build helpers.

use std::path::{Path, PathBuf};

use serde_json::{json, Value as JsonValue};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{build_program_image, BundleBuildReport};
use trust_runtime::native::{Target, Toolchain};

pub(crate) fn build_json_payload(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
) -> anyhow::Result<JsonValue> {
    let bundle_root = match bundle {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    // The project's program for this computer (specification 44 §8.8, NX-15); the toolchain is
    // found, not installed: `trust-runtime toolchain install` downloads it.
    let toolchain = Toolchain::find()?;
    let target = Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    let report = build_program_image(
        &bundle_root,
        sources.as_deref(),
        target,
        &toolchain,
        false,
        true,
    )?;
    Ok(build_payload_from_report(&bundle_root, report))
}

fn build_payload_from_report(bundle_root: &Path, report: BundleBuildReport) -> JsonValue {
    json!({
        "version": 1,
        "command": "build",
        "status": "ok",
        "project": bundle_root.display().to_string(),
        "program": report.image.display().to_string(),
        "source_count": report.sources.len(),
        "sources": report.sources.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "dependency_roots": report.dependency_roots.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "resolved_dependencies": report.resolved_dependencies,
    })
}
