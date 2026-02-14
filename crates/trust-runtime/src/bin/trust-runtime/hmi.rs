//! HMI scaffold command handlers.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::resolve_sources_root;
use trust_runtime::harness::{CompileSession, SourceFile as HarnessSourceFile};
use trust_runtime::hmi::{self, HmiScaffoldMode, HmiSourceRef};

use crate::cli::{HmiAction, HmiStyleArg};
use crate::style;

#[derive(Debug, Clone)]
struct LoadedSource {
    path: PathBuf,
    text: String,
}

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
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir()?,
        },
    };

    let sources_root = resolve_sources_root(&project_root, None)?;
    let sources = load_sources(&sources_root)?;
    if sources.is_empty() {
        anyhow::bail!("no ST sources found under {}", sources_root.display());
    }

    let compile_sources = sources
        .iter()
        .map(|source| {
            HarnessSourceFile::with_path(
                source.path.to_string_lossy().as_ref(),
                source.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let runtime = CompileSession::from_sources(compile_sources).build_runtime()?;
    let metadata = runtime.metadata_snapshot();
    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };

    let source_refs = sources
        .iter()
        .map(|source| HmiSourceRef {
            path: source.path.as_path(),
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

fn load_sources(root: &Path) -> anyhow::Result<Vec<LoadedSource>> {
    let mut paths = BTreeSet::new();
    for pattern in ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"] {
        for entry in glob::glob(&format!("{}/{}", root.display(), pattern))? {
            paths.insert(entry?);
        }
    }

    let mut sources = Vec::with_capacity(paths.len());
    for path in paths {
        let text = std::fs::read_to_string(&path)?;
        sources.push(LoadedSource { path, text });
    }
    Ok(sources)
}
