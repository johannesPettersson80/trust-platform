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
    let harness = TestHarness::from_source(source).expect("build harness");
    harness.runtime().metadata_snapshot()
}

fn scaffold_from_sources(root: &Path, style: &str, sources: &[(&str, &str)]) -> HmiScaffoldSummary {
    let compile_sources = sources
        .iter()
        .map(|(path, text)| HarnessSourceFile::with_path(*path, *text))
        .collect::<Vec<_>>();
    let runtime = CompileSession::from_sources(compile_sources)
        .build_runtime()
        .expect("build runtime");
    let metadata = runtime.metadata_snapshot();
    let snapshot = crate::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
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
    let runtime = CompileSession::from_sources(compile_sources)
        .build_runtime()
        .expect("build runtime");
    let metadata = runtime.metadata_snapshot();
    let snapshot = crate::debug::DebugSnapshot {
        storage_abi: Default::default(),
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
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
