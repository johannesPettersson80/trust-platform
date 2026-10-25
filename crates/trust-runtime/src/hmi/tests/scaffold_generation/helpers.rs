use super::*;
use crate::harness::{CompileSession, SourceFile as HarnessSourceFile, TestHarness};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

fn temp_dir(prefix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let dir = std::env::temp_dir().join(format!("{prefix}-{stamp}"));
    std::fs::create_dir_all(&dir).expect("create temp dir");
    dir
}

fn write_file(path: &Path, content: &str) {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create parent");
    }
    std::fs::write(path, content).expect("write file");
}

fn metadata_for_source(source: &str) -> RuntimeMetadata {
    CompileSession::from_source(source)
        .declarations()
        .expect("declare the source")
        .metadata()
        .clone()
}

/// The metadata and the starting values of `sources`, as `trust-runtime hmi` scaffolds from the
/// project's declarations (specification 44 §6.7).
fn declared_metadata_and_snapshot(
    sources: Vec<HarnessSourceFile>,
) -> (RuntimeMetadata, crate::debug::DebugSnapshot) {
    let declarations = CompileSession::from_sources(sources)
        .declarations()
        .expect("declare the sources");
    let metadata = declarations.metadata().clone();
    let snapshot = crate::debug::DebugSnapshot {
        storage: declarations.storage().clone(),
        now: crate::value::Duration::ZERO,
        compiled: None,
    };
    (metadata, snapshot)
}

fn scaffold_from_sources(root: &Path, style: &str, sources: &[(&str, &str)]) -> HmiScaffoldSummary {
    let compile_sources = sources
        .iter()
        .map(|(path, text)| HarnessSourceFile::with_path(*path, *text))
        .collect::<Vec<_>>();
    let (metadata, snapshot) = declared_metadata_and_snapshot(compile_sources);
    let loaded = sources
        .iter()
        .map(|(path, text)| (PathBuf::from(path), (*text).to_string()))
        .collect::<Vec<_>>();
    let refs = loaded
        .iter()
        .map(|(path, text)| HmiSourceRef {
            path: path.as_path(),
            text: text.as_str(),
        })
        .collect::<Vec<_>>();
    scaffold_hmi_dir_with_sources(root, &metadata, Some(&snapshot), &refs, style)
        .expect("scaffold hmi")
}

fn scaffold_from_sources_with_mode(
    root: &Path,
    style: &str,
    sources: &[(&str, &str)],
    mode: HmiScaffoldMode,
    force: bool,
) -> HmiScaffoldSummary {
    let compile_sources = sources
        .iter()
        .map(|(path, text)| HarnessSourceFile::with_path(*path, *text))
        .collect::<Vec<_>>();
    let (metadata, snapshot) = declared_metadata_and_snapshot(compile_sources);
    let loaded = sources
        .iter()
        .map(|(path, text)| (PathBuf::from(path), (*text).to_string()))
        .collect::<Vec<_>>();
    let refs = loaded
        .iter()
        .map(|(path, text)| HmiSourceRef {
            path: path.as_path(),
            text: text.as_str(),
        })
        .collect::<Vec<_>>();
    scaffold_hmi_dir_with_sources_mode(root, &metadata, Some(&snapshot), &refs, style, mode, force)
        .expect("scaffold hmi")
}
