//! The bundles of a program store: their labels, their copies, and the removal of the bundles that
//! no link names (specification 34 §11.2).

use std::collections::HashSet;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::{SystemTime, UNIX_EPOCH};

use super::{io_error, publish_atomic, refused, StoreError, NAME_SEQUENCE};
use crate::config::RuntimeConfig;
use crate::deployment::DEPLOYMENT_RECORD_FILE;

/// The program of a bundle.
pub(super) const PROGRAM_FILE: &str = "program.stbc";

/// A new label for a deployment, unique within this process and across restarts: the time, the
/// process id and a counter.
#[must_use]
pub fn default_bundle_label() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let sequence = NAME_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "project-{}-{:09}-{}-{sequence}",
        elapsed.as_secs(),
        elapsed.subsec_nanos(),
        std::process::id()
    )
}

/// Checks that `label` is one folder name: not empty, not a path, and without a drive prefix.
///
/// # Errors
///
/// The label is empty, contains a separator or a colon, or is not a single normal path component.
pub fn validate_bundle_label(label: &str) -> Result<(), StoreError> {
    let mut components = Path::new(label).components();
    let is_single_normal_component = matches!(
        (components.next(), components.next()),
        (Some(std::path::Component::Normal(component)), None)
            if component == std::ffi::OsStr::new(label)
    );
    if label.is_empty()
        || label.contains('/')
        || label.contains('\\')
        || label.contains(':')
        || !is_single_normal_component
    {
        return Err(refused(format!(
            "deployment label must be one non-empty filesystem name, not a path: {label:?}"
        )));
    }
    Ok(())
}

/// Checks that `path` is a normalized relative path inside a bundle: no empty, `.` or `..` part, no
/// backslash or colon, and no root. `field` names the setting in the error.
///
/// # Errors
///
/// The path is not a normalized relative path inside the bundle.
pub fn validate_bundle_relative_path(path: &Path, field: &str) -> Result<(), StoreError> {
    let display = path
        .to_str()
        .ok_or_else(|| refused(format!("{field} is not valid UTF-8")))?;
    let normalized = !display.is_empty()
        && !display.contains('\\')
        && !display.contains(':')
        && display
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if normalized {
        Ok(())
    } else {
        Err(refused(format!(
            "{field} must be a normalized relative path contained in the bundle: {}",
            path.display()
        )))
    }
}

/// Claims `label` for a new bundle in `bundles`. A bundle that a link in `in_use` names is refused
/// with `deployment already exists`. Any other folder of that label is left by an interrupted or
/// refused stage: no link names it, so it blocks nothing, and it is removed (specification 34 §11.2).
///
/// # Errors
///
/// The label is not valid, the bundle of a link has that label, or the leftover cannot be removed.
pub fn claim_label(bundles: &Path, label: &str, in_use: &[PathBuf]) -> Result<PathBuf, StoreError> {
    validate_bundle_label(label)?;
    let dest = bundles.join(label);
    if fs::symlink_metadata(&dest).is_ok() {
        let named = dest
            .canonicalize()
            .is_ok_and(|canonical| in_use.contains(&canonical));
        if named {
            return Err(refused(format!(
                "deployment already exists: {}",
                dest.display()
            )));
        }
        fs::remove_dir_all(&dest).map_err(|error| {
            io_error(
                format!("remove the interrupted deployment '{}'", dest.display()),
                error,
            )
        })?;
    }
    Ok(dest)
}

/// Copies a bundle's files and the sidecars its `runtime.toml` enables into `dest`. A copy that fails
/// removes `dest`.
///
/// # Errors
///
/// A file cannot be read or written, a path is a symbolic link, or a sidecar path is not a normalized
/// relative path.
pub fn copy_bundle(source: &Path, dest: &Path, runtime: &RuntimeConfig) -> Result<(), StoreError> {
    let result =
        copy_bundle_files(source, dest).and_then(|()| copy_sidecars(source, dest, runtime));
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}

/// Copies the files that every bundle carries: `runtime.toml` and `program.stbc`, and the optional
/// `io.toml`, `simulation.toml`, deployment record and `src` folder.
pub(super) fn copy_bundle_files(source: &Path, dest: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(dest)
        .map_err(|error| io_error(format!("create bundle '{}'", dest.display()), error))?;
    copy_file(&source.join("runtime.toml"), &dest.join("runtime.toml"))?;
    copy_optional_file(&source.join("io.toml"), &dest.join("io.toml"))?;
    copy_optional_file(
        &source.join("simulation.toml"),
        &dest.join("simulation.toml"),
    )?;
    copy_file(&source.join(PROGRAM_FILE), &dest.join(PROGRAM_FILE))?;
    copy_optional_file(
        &source.join(DEPLOYMENT_RECORD_FILE),
        &dest.join(DEPLOYMENT_RECORD_FILE),
    )?;
    copy_optional_dir(&source.join("src"), &dest.join("src"))
}

/// Copies the sidecar files that `runtime` enables, `[runtime.ads]` and `[runtime.opcua_client]`,
/// from `source` into `dest` (specification 34 §11.2).
///
/// # Errors
///
/// A sidecar path is not a normalized relative path, a path is a symbolic link, or a file cannot be
/// copied.
pub fn copy_sidecars(
    source: &Path,
    dest: &Path,
    runtime: &RuntimeConfig,
) -> Result<(), StoreError> {
    if runtime.ads.enabled {
        copy_bundle_relative_file(
            source,
            dest,
            &runtime.ads.config_path,
            "runtime.ads.config_path",
        )?;
    }
    if runtime.opcua_client.enabled {
        copy_bundle_relative_file(
            source,
            dest,
            &runtime.opcua_client.config_path,
            "runtime.opcua_client.config_path",
        )?;
    }
    Ok(())
}

fn copy_bundle_relative_file(
    source_root: &Path,
    dest_root: &Path,
    relative: &Path,
    field: &str,
) -> Result<(), StoreError> {
    validate_bundle_relative_path(relative, field)?;
    reject_symlink_components(source_root, relative)?;
    copy_file(&source_root.join(relative), &dest_root.join(relative))
}

fn reject_symlink_components(root: &Path, relative: &Path) -> Result<(), StoreError> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            return Err(refused(format!(
                "bundle path '{}' is not normalized",
                relative.display()
            )));
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path).map_err(|error| {
            io_error(format!("inspect bundle path '{}'", path.display()), error)
        })?;
        if metadata.file_type().is_symlink() {
            return Err(refused(format!(
                "bundle path '{}' is a symbolic link",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Copies one file to `dest` as `publish_atomic` writes a file: through a temporary file that is
/// synced, a rename, and a sync of the folder (G17). A symbolic link is refused.
fn copy_file(source: &Path, dest: &Path) -> Result<(), StoreError> {
    let metadata = fs::symlink_metadata(source)
        .map_err(|error| io_error(format!("inspect bundle file '{}'", source.display()), error))?;
    if metadata.file_type().is_symlink() {
        return Err(refused(format!(
            "bundle path '{}' is a symbolic link",
            source.display()
        )));
    }
    if !metadata.is_file() {
        return Err(refused(format!("missing file {}", source.display())));
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)
            .map_err(|error| io_error(format!("create '{}'", parent.display()), error))?;
    }
    publish_atomic(dest, |file| {
        let mut input = fs::File::open(source)?;
        io::copy(&mut input, file).map(|_| ())
    })
    .map_err(|error| {
        io_error(
            format!("copy '{}' to '{}'", source.display(), dest.display()),
            error,
        )
    })
}

fn copy_optional_file(source: &Path, dest: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(refused(format!(
            "bundle path '{}' is a symbolic link",
            source.display()
        ))),
        Ok(metadata) if metadata.is_file() => copy_file(source, dest),
        Ok(_) => Err(refused(format!(
            "bundle path '{}' is not a file",
            source.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(
            format!("inspect bundle file '{}'", source.display()),
            error,
        )),
    }
}

fn copy_optional_dir(source: &Path, dest: &Path) -> Result<(), StoreError> {
    match fs::symlink_metadata(source) {
        Ok(metadata) if metadata.file_type().is_symlink() => Err(refused(format!(
            "bundle path '{}' is a symbolic link",
            source.display()
        ))),
        Ok(metadata) if metadata.is_dir() => copy_dir(source, dest),
        Ok(_) => Err(refused(format!(
            "bundle path '{}' is not a directory",
            source.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io_error(
            format!("inspect bundle directory '{}'", source.display()),
            error,
        )),
    }
}

fn copy_dir(source: &Path, dest: &Path) -> Result<(), StoreError> {
    fs::create_dir_all(dest)
        .map_err(|error| io_error(format!("create '{}'", dest.display()), error))?;
    let entries = fs::read_dir(source).map_err(|error| {
        io_error(
            format!("read bundle directory '{}'", source.display()),
            error,
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            io_error(
                format!("read bundle directory '{}'", source.display()),
                error,
            )
        })?;
        let path = entry.path();
        let target = dest.join(entry.file_name());
        let file_type = entry.file_type().map_err(|error| {
            io_error(format!("inspect bundle path '{}'", path.display()), error)
        })?;
        if file_type.is_symlink() {
            return Err(refused(format!(
                "bundle path '{}' is a symbolic link",
                path.display()
            )));
        }
        if file_type.is_dir() {
            copy_dir(&path, &target)?;
        } else if file_type.is_file() {
            copy_file(&path, &target)?;
        } else {
            return Err(refused(format!(
                "bundle path '{}' has an unsupported file type",
                path.display()
            )));
        }
    }
    Ok(())
}

/// Removes every folder of `bundles_dir` that is not one of `keep`: after a deployment, no bundle
/// except `current` and `previous` is kept (specification 34 §11.2).
///
/// # Errors
///
/// The folder cannot be read, or a bundle cannot be removed.
pub fn prune_bundles(bundles_dir: &Path, keep: &[PathBuf]) -> Result<(), StoreError> {
    if !bundles_dir.is_dir() {
        return Ok(());
    }
    let keep_set = keep
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<HashSet<_>>();
    let entries = fs::read_dir(bundles_dir).map_err(|error| {
        io_error(
            format!("read bundle store '{}'", bundles_dir.display()),
            error,
        )
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            io_error(
                format!("read bundle store '{}'", bundles_dir.display()),
                error,
            )
        })?;
        let file_type = entry
            .file_type()
            .map_err(|error| io_error(format!("inspect '{}'", entry.path().display()), error))?;
        if !file_type.is_dir() {
            continue;
        }
        let path = entry.path();
        let canonical = path.canonicalize().unwrap_or_else(|_| path.clone());
        if keep_set.contains(&canonical) {
            continue;
        }
        fs::remove_dir_all(&path)
            .map_err(|error| io_error(format!("remove bundle '{}'", path.display()), error))?;
    }
    Ok(())
}

/// The bundles a store keeps after a deployment: `current` and the bundle before it.
#[must_use]
pub fn bundle_targets(current: &Path, previous: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut targets = vec![current.to_path_buf()];
    if let Some(previous) = previous {
        targets.push(previous.clone());
    }
    targets
}
