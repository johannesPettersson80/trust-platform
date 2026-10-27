//! Bounded open-document snapshot shared by HMI engineering commands.
use std::{
    collections::BTreeMap,
    path::{Component, Path, PathBuf},
};

pub(super) fn paths(
    root: &Path,
    open: &BTreeMap<String, String>,
) -> Result<BTreeMap<PathBuf, String>, String> {
    if open.len() > 4096 || open.values().map(String::len).sum::<usize>() > 64 * 1024 * 1024 {
        return Err("The open project snapshot exceeds its size limit".into());
    }
    let mut result = BTreeMap::new();
    let resolved_root = super::client_paths::resolved_root(root);
    for (file, text) in open {
        let path = Path::new(file);
        if file.contains('\\')
            || path
                .components()
                .any(|part| !matches!(part, Component::Normal(_)))
            || path.as_os_str().is_empty()
        {
            return Err("Open project files must use project-relative paths".into());
        }
        // The file keeps the client's path; where it lies is decided by its resolved path, which
        // for a new file is the resolved folder that will hold it.
        let path = root.join(path);
        if !super::client_paths::resolved(&path).starts_with(&resolved_root) {
            return Err("Open project files must belong to this project".into());
        }
        result.insert(path, text.clone());
    }
    Ok(result)
}
