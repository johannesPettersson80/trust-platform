//! Open manifests are resolved by the existing dependency checker using a temporary
//! metadata-only view. No checkout, ST source, runtime or project file is rewritten.
use std::collections::{BTreeMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use trust_runtime::harness::SourceFile;

const MANIFESTS: [&str; 3] = ["trust-lsp.toml", ".trust-lsp.toml", "trustlsp.toml"];
struct MetadataView(PathBuf);
impl Drop for MetadataView {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn temporary() -> Result<MetadataView, String> {
    static NEXT: AtomicU64 = AtomicU64::new(0);
    let path = std::env::temp_dir().join(format!(
        "trust-hmi-manifests-{}-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_err(|e| e.to_string())?
            .as_nanos(),
        NEXT.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir(&path).map_err(|e| e.to_string())?;
    Ok(MetadataView(path))
}
fn manifest(root: &Path, open: &BTreeMap<PathBuf, String>) -> Result<toml::Table, String> {
    let Some(path) = MANIFESTS
        .iter()
        .map(|n| root.join(n))
        .find(|p| open.contains_key(p) || p.is_file())
    else {
        return Ok(toml::Table::new());
    };
    let text = match open.get(&path) {
        Some(t) => t.clone(),
        None => std::fs::read_to_string(&path).map_err(|e| e.to_string())?,
    };
    toml::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))
}
fn dependency_path(value: &toml::Value) -> Result<&str, String> {
    value
        .as_str()
        .or_else(|| value.get("path").and_then(toml::Value::as_str))
        .ok_or("A dependency needs its local package path".into())
}
/// The local library roots of the project at `root`, those that an unsaved manifest of `open` names
/// included, under the paths the client names them by when they lie in the project, as the open
/// documents are named.
pub(crate) fn dependency_roots(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
) -> Result<Vec<PathBuf>, String> {
    let resolved_root = super::super::client_paths::resolved_root(root);
    Ok(resolved_dependency_roots(root, open)?
        .into_iter()
        .map(|library| super::super::client_paths::client_path(root, &resolved_root, library))
        .collect())
}

/// The library roots as the build resolves them.
fn resolved_dependency_roots(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
) -> Result<Vec<PathBuf>, String> {
    if !open.keys().any(|p| {
        p.file_name()
            .and_then(|n| n.to_str())
            .is_some_and(|n| MANIFESTS.contains(&n))
    }) {
        return trust_runtime::bundle_builder::dependency_roots(root).map_err(|e| e.to_string());
    }
    // The unsaved files under the names the resolved roots give them (through symbolic links).
    let open: BTreeMap<PathBuf, String> = open
        .iter()
        .map(|(path, text)| (super::super::client_paths::resolved(path), text.clone()))
        .collect();
    let view = temporary()?;
    // The snapshot's folder as the dependency roots name it: resolved through symbolic links, as a
    // temporary folder behind one is (`/var` on macOS), so that each root maps back to its folder.
    let snapshot = view.0.canonicalize().map_err(|e| e.to_string())?;
    // The project's and the libraries' folders in the form the unsaved files above are named by.
    let root = trust_runtime::runtime_sources::canonicalize_lossy(root);
    let mut ids = BTreeMap::from([(root.clone(), 0usize)]);
    let mut pending = VecDeque::from([root]);
    while let Some(real) = pending.pop_front() {
        if ids.len() > 256 {
            return Err("An editing snapshot supports at most 256 local library roots".into());
        }
        let id = ids[&real];
        let folder = snapshot.join(id.to_string());
        std::fs::create_dir_all(folder.join("src")).map_err(|e| e.to_string())?;
        let mut table = manifest(&real, &open)?;
        if let Some(dependencies) = table
            .get_mut("dependencies")
            .and_then(toml::Value::as_table_mut)
        {
            for (_, value) in dependencies.iter_mut() {
                let target = trust_runtime::runtime_sources::canonicalize_lossy(
                    &real.join(dependency_path(value)?),
                );
                if !target.join("src").is_dir() {
                    return Err(format!(
                        "Dependency {} has no src directory",
                        target.display()
                    ));
                }
                let next = ids.len();
                let target_id = *ids.entry(target.clone()).or_insert_with(|| {
                    pending.push_back(target);
                    next
                });
                let path = toml::Value::String(
                    snapshot
                        .join(target_id.to_string())
                        .to_string_lossy()
                        .into_owned(),
                );
                if value.is_str() {
                    *value = path;
                } else {
                    value
                        .as_table_mut()
                        .ok_or("Invalid dependency")?
                        .insert("path".into(), path);
                }
            }
        }
        std::fs::write(
            folder.join("trust-lsp.toml"),
            toml::to_string(&table).map_err(|e| e.to_string())?,
        )
        .map_err(|e| e.to_string())?;
    }
    let result =
        trust_runtime::bundle_builder::dependency_roots(&snapshot.join("0")).map_err(|e| {
            let mut message = e.to_string();
            for (real, id) in &ids {
                message = message.replace(
                    &snapshot.join(id.to_string()).to_string_lossy().to_string(),
                    &real.to_string_lossy(),
                );
            }
            message
        })?;
    result
        .into_iter()
        .map(|p| {
            ids.iter()
                .find(|(_, id)| snapshot.join(id.to_string()) == p)
                .map(|(real, _)| real.clone())
                .ok_or("Dependency snapshot mapping disappeared".into())
        })
        .collect()
}
pub(crate) fn collect(
    root: &Path,
    open: &BTreeMap<PathBuf, String>,
) -> Result<Vec<SourceFile>, String> {
    let sources = if root.join("src").is_dir() {
        trust_runtime::bundle_builder::resolve_sources_root(root, None)
            .map_err(|e| e.to_string())?
    } else if !root.join("src").exists() && !root.join("sources").exists() {
        root.join("src") // A design may precede its first PLC source file.
    } else {
        return Err("The project needs a valid src directory".into());
    };
    // The build resolves the folders through symbolic links; the files keep the client's paths,
    // which the open documents and the edits of the commands use.
    let resolved_root = super::super::client_paths::resolved_root(root);
    let client =
        |path: PathBuf| super::super::client_paths::client_path(root, &resolved_root, path);
    let mut roots = vec![client(sources)];
    roots.extend(
        dependency_roots(root, open)?
            .into_iter()
            .map(|r| r.join("src")),
    );
    // An empty metadata root lets the native collector scan each already-resolved source
    // tree without re-reading a stale on-disk dependency graph.
    let view = temporary()?;
    let mut files = BTreeMap::new();
    for source_root in &roots {
        if !source_root.exists() {
            continue;
        }
        for mut file in
            trust_runtime::bundle_builder::collect_project_source_files(&view.0, Some(source_root))
                .map_err(|e| e.to_string())?
        {
            let path = client(PathBuf::from(
                file.path
                    .as_deref()
                    .ok_or("Collected ST source has no path")?,
            ));
            file.path = Some(path.to_string_lossy().into_owned());
            if let Some(text) = open.get(&path) {
                file.text = text.clone();
            }
            files.insert(path, file);
        }
    }
    for (path, text) in open {
        if path
            .extension()
            .is_some_and(|s| s.eq_ignore_ascii_case("st"))
            && roots.iter().any(|r| path.starts_with(r))
        {
            files.insert(
                path.clone(),
                SourceFile::with_path(path.to_string_lossy().into_owned(), text.clone()),
            );
        }
    }
    Ok(files.into_values().collect())
}
