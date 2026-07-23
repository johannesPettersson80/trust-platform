//! HMI scaffold command handlers.

use std::path::PathBuf;

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::{collect_project_source_files, resolve_sources_root};
use trust_runtime::harness::CompileSession;
use trust_runtime::hmi::{self, HmiScaffoldMode, HmiSourceRef};

use crate::cli::{HmiAction, HmiStyleArg};
use crate::style;

pub fn run_hmi(project: Option<PathBuf>, action: HmiAction) -> anyhow::Result<()> {
    match action {
        HmiAction::Init { style, force } => {
            run_hmi_scaffold(project, style, HmiScaffoldMode::Init, force)
        }
        HmiAction::Update { style } => {
            run_hmi_scaffold(project, style, HmiScaffoldMode::Update, false)
        }
        HmiAction::Reset { style } => {
            run_hmi_scaffold(project, style, HmiScaffoldMode::Reset, false)
        }
    }
}

fn run_hmi_scaffold(
    project: Option<PathBuf>,
    style: HmiStyleArg,
    mode: HmiScaffoldMode,
    force: bool,
) -> anyhow::Result<()> {
    let project_root = match project {
        Some(path) => path,
        None => detect_bundle_path(None)?,
    };

    let sources_root = resolve_sources_root(&project_root, None)?;
    let sources = collect_project_source_files(&project_root, None)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let source_paths = sources
        .iter()
        .map(|source| {
            source
                .path
                .as_deref()
                .map(PathBuf::from)
                .ok_or_else(|| anyhow::anyhow!("project source is missing path metadata"))
        })
        .collect::<anyhow::Result<Vec<_>>>()?;
    let runtime = CompileSession::from_sources(sources.clone()).build_runtime()?;
    let metadata = runtime.metadata_snapshot();
    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };

    let source_refs = sources
        .iter()
        .zip(&source_paths)
        .map(|(source, path)| HmiSourceRef {
            path: path.as_path(),
            text: source.text.as_str(),
        })
        .collect::<Vec<_>>();

    let summary = hmi::scaffold_hmi_dir_with_sources_mode(
        project_root.as_path(),
        &metadata,
        Some(&snapshot),
        &source_refs,
        style.as_str(),
        mode,
        force,
    )?;

    println!(
        "{}",
        style::success(format!(
            "Generated HMI scaffold in {} ({})",
            project_root.join("hmi").display(),
            mode.as_str()
        ))
    );
    println!("{}", summary.render_text());
    Ok(())
}
