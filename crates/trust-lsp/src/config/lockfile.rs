use std::collections::BTreeMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};

pub(super) fn dependency_lock_path(root: &Path, build: &super::BuildConfig) -> PathBuf {
    if build.dependency_lockfile.is_absolute() {
        build.dependency_lockfile.clone()
    } else {
        root.join(&build.dependency_lockfile)
    }
}

pub(super) fn load_dependency_lock(path: &Path) -> Result<super::DependencyLockFile, String> {
    if !path.is_file() {
        return Ok(super::DependencyLockFile::default());
    }
    let content = std::fs::read_to_string(path).map_err(|err| {
        format!(
            "failed to read dependency lock file {}: {err}",
            path.display()
        )
    })?;
    let lock: super::DependencyLockFile = toml::from_str(&content).map_err(|err| {
        format!(
            "failed to parse dependency lock file {}: {err}",
            path.display()
        )
    })?;
    if lock.version != dependency_lock_version() {
        return Err(format!(
            "unsupported dependency lock file version {} in {} (expected {})",
            lock.version,
            path.display(),
            dependency_lock_version()
        ));
    }
    Ok(lock)
}

pub(super) fn write_dependency_lock(
    path: &Path,
    dependencies: BTreeMap<String, super::DependencyLockEntry>,
) -> Result<(), String> {
    let lock = super::DependencyLockFile {
        version: dependency_lock_version(),
        dependencies,
    };
    let content = toml::to_string_pretty(&lock)
        .map_err(|err| format!("failed to encode dependency lock file: {err}"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| {
            format!(
                "failed to create dependency lock parent {}: {err}",
                parent.display()
            )
        })?;
    }
    std::fs::write(path, content).map_err(|err| {
        format!(
            "failed to write dependency lock file {}: {err}",
            path.display()
        )
    })
}

pub(super) fn dependency_lock_version() -> u32 {
    1
}

pub(super) fn sanitize_for_path(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('-');
        }
    }
    out.trim_matches('-').to_string()
}

pub(super) fn stable_hash_hex(value: &str) -> String {
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    value.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}
