//! Bundle build command (compile sources to program.stbc).

use std::path::{Path, PathBuf};

use indicatif::{ProgressBar, ProgressStyle};
use serde_json::{json, Value as JsonValue};
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{build_program_stbc, BundleBuildReport};
use trust_runtime::deployment::RecordWrite;
use trust_runtime_services::status::program_memory_json;

use crate::style;

fn build_payload_from_report(bundle_root: &Path, report: BundleBuildReport) -> JsonValue {
    let deployment_record = match &report.deployment_record {
        RecordWrite::Written(path) => JsonValue::String(path.display().to_string()),
        RecordWrite::Skipped(_) => JsonValue::Null,
    };
    json!({
        "version": 1,
        "command": "build",
        "status": "ok",
        "project": bundle_root.display().to_string(),
        "program": report.program_path.display().to_string(),
        "deployment_record": deployment_record,
        "program_memory": report.program_memory.as_ref().map(program_memory_json),
        "source_count": report.sources.len(),
        "sources": report.sources.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "dependency_roots": report.dependency_roots.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        "resolved_dependencies": report.resolved_dependencies,
    })
}

pub fn run_build(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
    ci: bool,
) -> anyhow::Result<()> {
    let bundle_root = match bundle {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let report = if ci {
        build_program_stbc(&bundle_root, sources.as_deref())?
    } else {
        let spinner = ProgressBar::new_spinner();
        spinner.set_style(ProgressStyle::default_spinner().template("{spinner} {msg}")?);
        spinner.enable_steady_tick(std::time::Duration::from_millis(120));
        spinner.set_message("Building program.stbc...");
        let report = build_program_stbc(&bundle_root, sources.as_deref())?;
        spinner.finish_and_clear();
        report
    };
    // Standard output stays the CI payload; the reason for a missing record goes to stderr.
    if let RecordWrite::Skipped(reason) = &report.deployment_record {
        eprintln!("warning: no deployment record written: {reason}");
    }
    if ci {
        let payload = build_payload_from_report(&bundle_root, report);
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    println!(
        "{}",
        style::success(format!("Wrote {}", report.program_path.display()))
    );
    if let RecordWrite::Written(path) = &report.deployment_record {
        println!("{}", style::success(format!("Wrote {}", path.display())));
    }
    if let Some(memory) = &report.program_memory {
        let figure = &memory.figure;
        println!(
            "Memory figure: {} bytes on this computer (program {}, persistent block {}, activation \
             stack {}, operand stack {}, engine {}, retain capture {}); each device computes its own",
            memory.bytes(),
            figure.program,
            figure.persistent_block,
            figure.activation_stack,
            figure.operand_stack,
            figure.engine,
            memory.retain_capture(),
        );
    }
    println!("Sources: {} file(s)", report.sources.len());
    for path in report.sources.iter().take(5) {
        println!(" - {}", path.display());
    }
    if report.sources.len() > 5 {
        println!(" - ... +{}", report.sources.len() - 5);
    }
    Ok(())
}
