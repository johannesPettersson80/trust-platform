/// The retain file of a project whose `[runtime.retain] path` is `configured` (specification 11,
/// section 6.7): an absolute path as it is, and a relative one in the folder of the project's
/// runtime files, the project folder or, for a runtime started on a program store's `current` link,
/// the store folder (section 6.8.0a). An install, a deployment or a rollback moves `current` to
/// another bundle, so a retain file kept in a bundle would be left behind with the bundle and the
/// next warm start would find none.
#[must_use]
pub fn retain_file_path(project_root: &Path, configured: &Path) -> PathBuf {
    if configured.is_absolute() {
        configured.to_path_buf()
    } else {
        crate::control::endpoint::runtime_files_folder(project_root).join(configured)
    }
}

/// [`retain_file_path`] for the start of a runtime. A store runtime's retain file that an earlier
/// version kept beside `runtime.toml` in the `current` bundle is moved into the store folder first,
/// unless the store folder already holds one, so that the upgrade keeps the retained values.
///
/// # Errors
///
/// The retain file cannot be moved into the store folder.
pub fn prepare_retain_file(project_root: &Path, configured: &Path) -> Result<PathBuf, RuntimeError> {
    let path = retain_file_path(project_root, configured);
    let in_bundle = project_root.join(configured);
    if configured.is_relative() && in_bundle != path && !path.exists() && in_bundle.is_file() {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|err| {
                RuntimeError::RetainStore(format!("create retain dir {parent:?}: {err}").into())
            })?;
        }
        fs::rename(&in_bundle, &path).map_err(|err| {
            RuntimeError::RetainStore(
                format!("move retain {in_bundle:?} into the store as {path:?}: {err}").into(),
            )
        })?;
    }
    Ok(path)
}
