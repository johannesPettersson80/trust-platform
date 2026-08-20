fn ensure_registry_layout(registry_root: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(registry_root)
        .with_context(|| format!("failed to create registry root {}", registry_root.display()))?;
    fs::create_dir_all(registry_packages_path(registry_root)).with_context(|| {
        format!(
            "failed to create registry packages directory {}",
            registry_packages_path(registry_root).display()
        )
    })?;
    Ok(())
}

fn ensure_empty_output_dir(output_root: &Path) -> anyhow::Result<()> {
    if output_root.is_file() {
        anyhow::bail!("output path is a file: {}", output_root.display());
    }
    if output_root.is_dir() {
        let has_entries = fs::read_dir(output_root)
            .with_context(|| format!("failed to read {}", output_root.display()))?
            .next()
            .is_some();
        if has_entries {
            anyhow::bail!("output directory is not empty: {}", output_root.display());
        }
        return Ok(());
    }
    fs::create_dir_all(output_root).with_context(|| {
        format!(
            "failed to create output directory {}",
            output_root.display()
        )
    })?;
    Ok(())
}

fn copy_dir_recursive(source: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)
        .with_context(|| format!("failed to create destination {}", dest.display()))?;
    for entry in
        fs::read_dir(source).with_context(|| format!("failed to read {}", source.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("failed to inspect {}", path.display()))?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!(
                "registry package trees must not contain symlinks: {}",
                path.display()
            );
        }
        let file_name = entry.file_name();
        let target = dest.join(file_name);
        if metadata.is_dir() {
            copy_dir_recursive(&path, &target)?;
        } else if metadata.is_file() {
            fs::copy(&path, &target).with_context(|| {
                format!(
                    "failed to copy '{}' -> '{}'",
                    path.display(),
                    target.display()
                )
            })?;
        }
    }
    Ok(())
}

fn validate_tree_has_no_symlinks(root: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(root)
        .with_context(|| format!("failed to inspect {}", root.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!(
            "registry package trees must not contain symlinks: {}",
            root.display()
        );
    }
    if !metadata.is_dir() {
        return Ok(());
    }
    for entry in fs::read_dir(root).with_context(|| format!("failed to read {}", root.display()))? {
        validate_tree_has_no_symlinks(&entry?.path())?;
    }
    Ok(())
}
