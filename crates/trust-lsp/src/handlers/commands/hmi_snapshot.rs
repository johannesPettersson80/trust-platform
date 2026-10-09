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
        let path = root.join(path);
        let path = path.canonicalize().unwrap_or(path);
        let canonical = root.canonicalize().unwrap_or_else(|_| root.to_path_buf());
        if !path.starts_with(&canonical) {
            return Err("Open project files must belong to this project".into());
        }
        result.insert(path, text.clone());
    }
    Ok(result)
}
