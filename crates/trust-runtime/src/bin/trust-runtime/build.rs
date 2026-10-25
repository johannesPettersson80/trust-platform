//! Bundle build command (specification 44 §4.1, §8.8): the project compiled into this computer's
//! program image `program.tpi`, or a device's image with `--target`.

use std::path::{Path, PathBuf};

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{build_program_image, BundleBuildReport};

use crate::style;

/// `trust-runtime build` (specification 44 §8.8, NX-15): this computer's program image, or a
/// device's with `--target`, with the toolchain, installed on the first build that needs it (§4.6).
pub fn run_build_command(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
    ci: bool,
    target: Option<String>,
    emit_source: bool,
) -> anyhow::Result<()> {
    match target {
        Some(target) => run_native_build(bundle, sources, &target, emit_source, ci, false),
        None => run_native_build(bundle, sources, "host", emit_source, ci, true),
    }
}

/// The target `name` names: a device, a CPU, a Rust target triple or `host` (specification 44 §4.4).
pub(crate) fn native_target(name: &str) -> anyhow::Result<trust_runtime::native::Target> {
    trust_runtime::native::target_named(name)
        .ok_or_else(|| anyhow::anyhow!("unknown target '{name}'"))
}

/// Builds the project's program for this computer as `trust-runtime build` does without options
/// (specification 44 NX-15).
pub(crate) fn build_project_program(bundle_root: &Path) -> anyhow::Result<()> {
    write_native_image(bundle_root, None, native_target("host")?, false, true).map(drop)
}

/// `trust-runtime build --target`: the project compiled into a program image in its build folder,
/// with the memory figure in `build.json` (specification 44 §4.1). The image of the computer that
/// runs the project, `for_this_computer`, is the project's program instead: `program.tpi` with
/// `deployment-record.bin` (NX-15).
pub fn run_native_build(
    bundle: Option<PathBuf>,
    sources: Option<PathBuf>,
    target: &str,
    emit_source: bool,
    ci: bool,
    for_this_computer: bool,
) -> anyhow::Result<()> {
    let bundle_root = match bundle {
        Some(path) => path,
        None => detect_bundle_path(None).unwrap_or(std::env::current_dir()?),
    };
    let built = write_native_image(
        &bundle_root,
        sources.as_deref(),
        native_target(target)?,
        emit_source,
        for_this_computer,
    )?;
    if ci {
        println!("{}", serde_json::to_string_pretty(&built.json)?);
        return Ok(());
    }
    let figure = built.figure;
    println!(
        "{}",
        style::success(format!("Wrote {}", built.image.display()))
    );
    println!(
        "{} {} bytes: code {}, read-only data {}, starting memory {}",
        if built.debug {
            "Debug image"
        } else {
            "Release image (no debug hooks)"
        },
        built.image_bytes,
        figure.code,
        figure.rodata,
        figure.init
    );
    println!(
        "Memory figure: {} bytes (areas {}, stack {}, bookkeeping {}, retain capture {})",
        figure.total, figure.areas, figure.stack, figure.bookkeeping, figure.retain_capture
    );
    // The public key a device must trust to run the image (specification 44 §5.11).
    println!("Signed by the key {}", built.signer);
    println!("Sources: {} file(s)", built.sources.len());
    for path in built.sources.iter().take(5) {
        println!(" - {}", path.display());
    }
    if built.sources.len() > 5 {
        println!(" - ... +{}", built.sources.len() - 5);
    }
    Ok(())
}

/// The build of [`run_native_build`], without its output.
fn write_native_image(
    bundle_root: &Path,
    sources: Option<&Path>,
    target: trust_runtime::native::Target,
    emit_source: bool,
    for_this_computer: bool,
) -> anyhow::Result<BundleBuildReport> {
    let toolchain = crate::toolchain::build_toolchain()?;
    build_program_image(
        bundle_root,
        sources,
        target,
        &toolchain,
        emit_source,
        for_this_computer,
    )
}
