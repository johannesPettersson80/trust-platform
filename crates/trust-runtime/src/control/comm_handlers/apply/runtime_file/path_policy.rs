use std::io::ErrorKind;
use std::path::{Component, Path, PathBuf};

pub(super) fn project_sidecar_path(
    project_root: &Path,
    configured_path: &str,
) -> Result<PathBuf, String> {
    let configured_path = configured_path.trim();
    if configured_path.is_empty() {
        return Err("Sidecar path must not be empty.".to_string());
    }

    let mut relative = PathBuf::new();
    for component in Path::new(configured_path).components() {
        match component {
            Component::Normal(segment) => relative.push(segment),
            Component::CurDir => {}
            Component::ParentDir => {
                if !relative.pop() {
                    return Err(
                        "Sidecar path must remain confined within the selected project."
                            .to_string(),
                    );
                }
            }
            Component::RootDir | Component::Prefix(_) => {
                return Err("Sidecar path must be relative to the selected project.".to_string());
            }
        }
    }
    if relative.as_os_str().is_empty() {
        return Err("Sidecar path must name a file below the selected project.".to_string());
    }

    let candidate = project_root.join(relative);
    let canonical_project = match project_root.canonicalize() {
        Ok(path) => path,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(candidate),
        Err(error) => return Err(format!("Failed to resolve selected project: {error}")),
    };
    if !canonical_project.is_dir() {
        return Err("Selected project root must be a directory.".to_string());
    }

    let mut existing = candidate.as_path();
    loop {
        match std::fs::symlink_metadata(existing) {
            Ok(_) => break,
            Err(error) if error.kind() == ErrorKind::NotFound => {
                existing = existing
                    .parent()
                    .ok_or_else(|| "Sidecar path has no resolvable project parent.".to_string())?;
            }
            Err(error) => {
                return Err(format!(
                    "Failed to inspect sidecar path {}: {error}",
                    existing.display()
                ))
            }
        }
    }
    let canonical_existing = existing.canonicalize().map_err(|error| {
        format!(
            "Failed to resolve sidecar path parent {}: {error}",
            existing.display()
        )
    })?;
    if !canonical_existing.starts_with(&canonical_project) {
        return Err("Sidecar path must remain confined within the selected project.".to_string());
    }

    Ok(candidate)
}
