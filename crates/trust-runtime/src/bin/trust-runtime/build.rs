//! Bundle build command (specification 44 §4.1, §8.8): the project compiled into this computer's
//! program image `program.tpi`, a device's image with `--target`, or `program.stbc` for the bytecode
//! engine with `--execution-backend vm` and, until that engine is deleted, on a computer without the
//! toolchain.

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

/// `trust-runtime build` (specification 44 §8.8, NX-15): this computer's program image, a device's
/// with `--target`, or `program.stbc` with `--execution-backend vm`. Until the bytecode engine is
/// deleted (§15), a computer without the toolchain builds `program.stbc` and says so.
pub fn run_build_command(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
    ci: bool,
    target: Option<String>,
    emit_source: bool,
    execution_backend: Option<crate::cli::ExecutionBackendArg>,
) -> anyhow::Result<()> {
    if let Some(target) = target {
        return run_native_build(bundle, sources, &target, emit_source, ci, false);
    }
    if execution_backend.is_some() {
        return run_build(bundle, sources, ci);
    }
    match trust_runtime::native::Toolchain::from_env() {
        Ok(_) => run_native_build(bundle, sources, "host", emit_source, ci, true),
        Err(error) => {
            eprintln!(
                "note: {error}: building program.stbc for the bytecode engine instead of this \
                 computer's program image"
            );
            run_build(bundle, sources, ci)
        }
    }
}

/// The target `name` names: a device, a CPU, a Rust target triple or `host` (specification 44 §4.4).
pub(crate) fn native_target(name: &str) -> anyhow::Result<trust_runtime::native::Target> {
    trust_runtime::native::target_named(name)
        .ok_or_else(|| anyhow::anyhow!("unknown target '{name}'"))
}

/// `trust-runtime build --target`: the project compiled into a native program image in its build
/// folder, with the memory figure in `build.json` (specification 44 §4.1). The image of the computer
/// that runs the project, `for_this_computer`, is the project's program instead: `program.tpi` with
/// `deployment-record.bin`, where the project kept `program.stbc` (NX-15).
pub fn run_native_build(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
    target: &str,
    emit_source: bool,
    ci: bool,
    for_this_computer: bool,
) -> anyhow::Result<()> {
    use trust_runtime::native::{build, debug_hooks, triple, Options, Toolchain};
    let bundle_root = match bundle {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let target = native_target(target)?;
    let files = trust_runtime::bundle_builder::collect_project_sources_labelled(
        &bundle_root,
        sources.as_deref(),
    )?;
    // The project's `[build] debug` chooses a debug or a release image (specification 44 §13.2).
    let debug = debug_hooks(&bundle_root)?;
    let toolchain = Toolchain::from_env()?;
    let build_folder = bundle_root.join("build");
    let work = build_folder.join("native").join(triple(target));
    let built = build(
        &files,
        &Options {
            target,
            hooks: debug,
        },
        &toolchain,
        &work,
    )?;
    let (image, record) = if for_this_computer {
        (
            bundle_root.join(trust_runtime::native::PROGRAM_IMAGE_FILE),
            bundle_root.join("deployment-record.bin"),
        )
    } else {
        (
            build_folder.join(format!("program.{}.tpi", triple(target))),
            build_folder.join(format!("deployment-record.{}.bin", triple(target))),
        )
    };
    std::fs::create_dir_all(&build_folder)?;
    std::fs::write(&image, &built.image)?;
    // The record a device admits the image with (specification 44 §10.1).
    std::fs::write(
        &record,
        trust_runtime::deployment::image_record(&bundle_root, &built.image)?,
    )?;
    if !emit_source {
        let _ = std::fs::remove_file(work.join("module.rs"));
    }
    let figure = built.figure;
    let report = json!({
        "version": 1,
        "command": "build",
        "status": "ok",
        "target": triple(target),
        "debug": debug,
        "image": image.display().to_string(),
        "image_bytes": built.image.len(),
        "record": record.display().to_string(),
        "memory_figure": {
            "areas": figure.areas,
            "stack": figure.stack,
            "bookkeeping": figure.bookkeeping,
            "retain_capture": figure.retain_capture,
            "total": figure.total,
            "call_level": figure.call_level,
        },
        "flash": {
            "code": figure.code,
            "rodata": figure.rodata,
            "init": figure.init,
        },
    });
    std::fs::write(
        build_folder.join("build.json"),
        serde_json::to_string_pretty(&report)? + "\n",
    )?;
    if ci {
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    println!("{}", style::success(format!("Wrote {}", image.display())));
    println!(
        "{} {} bytes: code {}, read-only data {}, starting memory {}",
        if debug {
            "Debug image"
        } else {
            "Release image (no debug hooks)"
        },
        built.image.len(),
        figure.code,
        figure.rodata,
        figure.init
    );
    println!(
        "Memory figure: {} bytes (areas {}, stack {}, bookkeeping {}, retain capture {})",
        figure.total, figure.areas, figure.stack, figure.bookkeeping, figure.retain_capture
    );
    Ok(())
}
