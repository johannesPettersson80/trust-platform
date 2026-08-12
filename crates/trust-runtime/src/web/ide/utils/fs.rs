use super::*;

pub(in crate::web::ide) fn collect_workspace_files(
    root: &Path,
    relative: &Path,
    out: &mut Vec<String>,
) -> Result<(), IdeError> {
    let dir = root.join(relative);
    let entries = std::fs::read_dir(&dir)
        .map_err(|err| IdeError::new(IdeErrorKind::Internal, format!("read_dir failed: {err}")))?;
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let file_name = entry.file_name();
        let file_name = file_name.to_string_lossy();
        if file_name.starts_with('.') {
            continue;
        }
        let next_relative = if relative.as_os_str().is_empty() {
            PathBuf::from(file_name.as_ref())
        } else {
            relative.join(file_name.as_ref())
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            collect_workspace_files(root, &next_relative, out)?;
            continue;
        }
        if file_type.is_file() {
            out.push(next_relative.to_string_lossy().replace('\\', "/"));
        }
    }
    Ok(())
}

pub(in crate::web::ide) fn collect_source_files(
    root: &Path,
    relative: &Path,
    out: &mut Vec<String>,
) -> Result<(), IdeError> {
    let mut files = Vec::new();
    collect_workspace_files(root, relative, &mut files)?;
    for path in files {
        if path.to_ascii_lowercase().ends_with(".st") {
            out.push(path);
        }
    }
    Ok(())
}

pub(in crate::web::ide) fn collect_workspace_tree(
    root: &Path,
    relative: &Path,
) -> Result<Vec<IdeTreeNode>, IdeError> {
    let dir = root.join(relative);
    let mut entries = std::fs::read_dir(&dir)
        .map_err(|err| IdeError::new(IdeErrorKind::Internal, format!("read_dir failed: {err}")))?
        .flatten()
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());

    let mut nodes = Vec::new();
    for entry in entries {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        if name.starts_with('.') {
            continue;
        }
        let rel = if relative.as_os_str().is_empty() {
            PathBuf::from(&name)
        } else {
            relative.join(&name)
        };
        if file_type.is_symlink() {
            continue;
        }
        if file_type.is_dir() {
            let children = collect_workspace_tree(root, &rel)?;
            nodes.push(IdeTreeNode {
                name: name.clone(),
                path: rel.to_string_lossy().replace('\\', "/"),
                kind: "directory".to_string(),
                children,
            });
            continue;
        }
        if file_type.is_file() {
            nodes.push(IdeTreeNode {
                name,
                path: rel.to_string_lossy().replace('\\', "/"),
                kind: "file".to_string(),
                children: Vec::new(),
            });
        }
    }
    Ok(nodes)
}

#[cfg(test)]
#[path = "fs/contract_tests.rs"]
mod contract_tests;
