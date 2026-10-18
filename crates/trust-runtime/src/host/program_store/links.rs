//! The `current` and `previous` links of a program store, and how each one is replaced: a staged
//! link is renamed over the old one, so the link always names a whole link (specification 34 §11.2;
//! D3).

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{io_error, refused, scratch_path, StoreError};

/// The state of a link: absent, or a symbolic link to `PathBuf`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PointerState {
    /// No file or link is at the path.
    Missing,
    /// A symbolic link to this target, as the link holds it.
    Link(PathBuf),
}

/// Reads the state of the link at `path`.
///
/// # Errors
///
/// The path holds something other than a symbolic link, or the link cannot be read.
pub(super) fn pointer_state(path: &Path) -> Result<PointerState, StoreError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path).map_err(|error| {
                io_error(
                    format!("read deployment pointer '{}'", path.display()),
                    error,
                )
            })?;
            Ok(PointerState::Link(target))
        }
        Ok(_) => Err(refused(format!(
            "deployment pointer '{}' exists and is not a symbolic link",
            path.display()
        ))),
        Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(PointerState::Missing),
        Err(error) => Err(io_error(format!("inspect '{}'", path.display()), error)),
    }
}

/// Checks that `path` is free or a symbolic link, so that a deployment may replace it.
///
/// # Errors
///
/// The path holds something other than a symbolic link.
pub fn validate_pointer_slot(path: &Path) -> Result<(), StoreError> {
    pointer_state(path).map(|_| ())
}

/// The target of the link at `path`, resolved against the link's folder when it is relative. `None`
/// when no link is at the path.
///
/// # Errors
///
/// The path holds something other than a symbolic link, or the link cannot be read.
pub fn read_link_target(path: &Path) -> Result<Option<PathBuf>, StoreError> {
    let target = match pointer_state(path)? {
        PointerState::Missing => return Ok(None),
        PointerState::Link(target) => target,
    };
    if target.is_absolute() {
        return Ok(Some(target));
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Ok(Some(parent.join(target)))
}

/// The bundle that the link at `link` names, as a canonical path inside `bundles_dir`. `None` when
/// no link is at the path, or when its target is missing.
///
/// # Errors
///
/// The target lies outside `bundles_dir`, is not a folder, or the store cannot be resolved.
pub fn resolve_existing_bundle_link(
    bundles_dir: &Path,
    link: &Path,
) -> Result<Option<PathBuf>, StoreError> {
    let Some(target) = read_link_target(link)? else {
        return Ok(None);
    };
    let target = match target.canonicalize() {
        Ok(target) => target,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(io_error(
                format!("resolve deployment pointer '{}'", link.display()),
                error,
            ));
        }
    };
    let bundle_store = bundles_dir.canonicalize().map_err(|error| {
        io_error(
            format!(
                "resolve deployment bundle store '{}'",
                bundles_dir.display()
            ),
            error,
        )
    })?;
    if !target.is_dir() || target.parent() != Some(bundle_store.as_path()) {
        return Err(refused(format!(
            "deployment pointer '{}' resolves outside deployment bundle store '{}': {}",
            link.display(),
            bundle_store.display(),
            target.display()
        )));
    }
    Ok(Some(target))
}

fn create_directory_symlink(target: &Path, link: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

/// Replaces the link at `link` with one to `target`. The new link is staged beside it and renamed
/// over it, so a stop leaves the old link or the new one, never no link (specification 34 §11.2; D3).
///
/// # Errors
///
/// The path holds something other than a symbolic link, the new link cannot be staged, or the
/// rename fails.
pub fn replace_symlink(link: &Path, target: &Path) -> Result<(), StoreError> {
    validate_pointer_slot(link)?;
    let staged = scratch_path(link, "new");
    create_directory_symlink(target, &staged).map_err(|error| {
        io_error(
            format!("stage deployment pointer '{}'", link.display()),
            error,
        )
    })?;
    #[cfg(unix)]
    {
        if let Err(error) = fs::rename(&staged, link) {
            let _ = fs::remove_file(&staged);
            return Err(io_error(
                format!("publish deployment pointer '{}'", link.display()),
                error,
            ));
        }
    }
    #[cfg(windows)]
    {
        publish_on_windows(link, &staged)?;
    }
    Ok(())
}

/// Windows cannot rename a link over another, so the old link is moved aside first and restored if
/// the new one cannot be put in its place.
#[cfg(windows)]
fn publish_on_windows(link: &Path, staged: &Path) -> Result<(), StoreError> {
    let backup = scratch_path(link, "old");
    let had_link = matches!(pointer_state(link)?, PointerState::Link(_));
    if had_link && let Err(error) = fs::rename(link, &backup) {
        let _ = fs::remove_file(staged);
        return Err(io_error(
            format!("backup deployment pointer '{}'", link.display()),
            error,
        ));
    }
    if let Err(error) = fs::rename(staged, link) {
        let restore = if had_link {
            fs::rename(&backup, link)
        } else {
            Ok(())
        };
        let _ = fs::remove_file(staged);
        if let Err(restore_error) = restore {
            return Err(refused(format!(
                "publish deployment pointer '{}' ({error}) and restore its backup: {restore_error}",
                link.display()
            )));
        }
        return Err(io_error(
            format!("publish deployment pointer '{}'", link.display()),
            error,
        ));
    }
    if had_link {
        fs::remove_dir(&backup).map_err(|error| {
            io_error(
                format!("remove deployment pointer backup '{}'", backup.display()),
                error,
            )
        })?;
    }
    Ok(())
}

/// Puts the link at `link` back into `state`: a missing link is removed, and a link is replaced.
pub(super) fn set_pointer_state(link: &Path, state: &PointerState) -> Result<(), StoreError> {
    match state {
        PointerState::Missing => match pointer_state(link)? {
            PointerState::Missing => Ok(()),
            PointerState::Link(_) => fs::remove_file(link).map_err(|error| {
                io_error(
                    format!("remove deployment pointer '{}'", link.display()),
                    error,
                )
            }),
        },
        PointerState::Link(target) => replace_symlink(link, target),
    }
}

/// Puts both links back into their states after a failed change. Both are tried, and the first
/// failure is reported.
///
/// # Errors
///
/// A link cannot be put back into its state.
pub fn restore_pointer_pair(
    current_link: &Path,
    current_state: &PointerState,
    previous_link: &Path,
    previous_state: &PointerState,
) -> Result<(), StoreError> {
    let current_result = set_pointer_state(current_link, current_state);
    let previous_result = set_pointer_state(previous_link, previous_state);
    match (current_result, previous_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(current_error), Err(previous_error)) => Err(refused(format!(
            "{current_error}; also failed to restore previous deployment pointer: {previous_error}"
        ))),
    }
}

/// Replaces `current_link` and then `previous_link`. When the second replacement fails, the first
/// is restored. Returns the states both links had before the change.
fn transition_pointer_pair(
    current_link: &Path,
    new_current: &PointerState,
    previous_link: &Path,
    new_previous: &PointerState,
) -> Result<(PointerState, PointerState), StoreError> {
    let old_current = pointer_state(current_link)?;
    let old_previous = pointer_state(previous_link)?;
    set_pointer_state(current_link, new_current)?;
    if let Err(error) = set_pointer_state(previous_link, new_previous) {
        if let Err(restore_error) =
            restore_pointer_pair(current_link, &old_current, previous_link, &old_previous)
        {
            return Err(refused(format!(
                "{error}; also failed to restore deployment pointers: {restore_error}"
            )));
        }
        return Err(error);
    }
    Ok((old_current, old_previous))
}

/// Commits a deployment: `current_link` names `new_current` and `previous_link` names the bundle that
/// `current_link` named before (`old_current`), or is removed when there was none. Returns the states
/// of both links before the change, for [`restore_pointer_pair`].
///
/// # Errors
///
/// The new bundle cannot be resolved, or a link cannot be replaced; the first link is restored when
/// the second fails.
pub fn update_deployment_pointers(
    current_link: &Path,
    previous_link: &Path,
    new_current: &Path,
    old_current: Option<&PathBuf>,
) -> Result<(PointerState, PointerState), StoreError> {
    let new_current = PointerState::Link(new_current.canonicalize().map_err(|error| {
        io_error(
            format!("resolve deployment target '{}'", new_current.display()),
            error,
        )
    })?);
    let new_previous = match old_current {
        Some(old_current) => PointerState::Link(old_current.clone()),
        None => PointerState::Missing,
    };
    transition_pointer_pair(current_link, &new_current, previous_link, &new_previous)
}
