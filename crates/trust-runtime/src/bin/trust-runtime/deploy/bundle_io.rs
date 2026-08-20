static BUNDLE_LABEL_SEQUENCE: AtomicU64 = AtomicU64::new(1);

fn validate_bundle(bundle: &RuntimeBundle) -> anyhow::Result<()> {
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        if !driver.enabled {
            continue;
        }
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    let mut runtime = trust_runtime::Runtime::new();
    runtime.apply_bytecode_bytes(&bundle.bytecode, Some(&bundle.runtime.resource_name))?;
    Ok(())
}

fn copy_bundle(bundle: &RuntimeBundle, dest: &Path) -> anyhow::Result<()> {
    let source = &bundle.root;
    let result = (|| {
        fs::create_dir_all(dest)?;
        copy_file(source.join("runtime.toml"), dest.join("runtime.toml"))?;
        copy_optional_file(source.join("io.toml"), dest.join("io.toml"))?;
        copy_optional_file(source.join("simulation.toml"), dest.join("simulation.toml"))?;
        copy_file(source.join("program.stbc"), dest.join("program.stbc"))?;

        if bundle.runtime.ads.enabled {
            copy_bundle_relative_file(
                source,
                dest,
                &bundle.runtime.ads.config_path,
                "runtime.ads.config_path",
            )?;
        }
        if bundle.runtime.opcua_client.enabled {
            copy_bundle_relative_file(
                source,
                dest,
                &bundle.runtime.opcua_client.config_path,
                "runtime.opcua_client.config_path",
            )?;
        }

        copy_optional_dir(source.join("src"), dest.join("src"))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_dir_all(dest);
    }
    result
}

fn copy_bundle_relative_file(
    source_root: &Path,
    dest_root: &Path,
    relative: &Path,
    field: &str,
) -> anyhow::Result<()> {
    validate_bundle_relative_path(relative, field)?;
    reject_symlink_components(source_root, relative)?;
    copy_file(source_root.join(relative), dest_root.join(relative))
}

fn validate_bundle_relative_path(path: &Path, field: &str) -> anyhow::Result<()> {
    let display = path
        .to_str()
        .with_context(|| format!("{field} is not valid UTF-8"))?;
    let normalized = !display.is_empty()
        && !display.contains('\\')
        && !display.contains(':')
        && display
            .split('/')
            .all(|component| !component.is_empty() && component != "." && component != "..")
        && path
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)));
    if !normalized {
        anyhow::bail!(
            "{field} must be a normalized relative path contained in the bundle: {}",
            path.display()
        );
    }
    Ok(())
}

fn reject_symlink_components(root: &Path, relative: &Path) -> anyhow::Result<()> {
    let mut path = root.to_path_buf();
    for component in relative.components() {
        let std::path::Component::Normal(component) = component else {
            unreachable!("relative bundle path was validated");
        };
        path.push(component);
        let metadata = fs::symlink_metadata(&path)
            .with_context(|| format!("inspect bundle path '{}'", path.display()))?;
        if metadata.file_type().is_symlink() {
            anyhow::bail!("bundle path '{}' is a symbolic link", path.display());
        }
    }
    Ok(())
}

fn copy_file(source: PathBuf, dest: PathBuf) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(&source)
        .with_context(|| format!("inspect bundle file '{}'", source.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!("bundle path '{}' is a symbolic link", source.display());
    }
    if !metadata.is_file() {
        anyhow::bail!("missing file {}", source.display());
    }
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::copy(&source, &dest)?;
    Ok(())
}

fn copy_optional_file(source: PathBuf, dest: PathBuf) -> anyhow::Result<()> {
    match fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("bundle path '{}' is a symbolic link", source.display())
        }
        Ok(metadata) if metadata.is_file() => copy_file(source, dest),
        Ok(_) => anyhow::bail!("bundle path '{}' is not a file", source.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("inspect bundle file '{}'", source.display()))
        }
    }
}

fn copy_optional_dir(source: PathBuf, dest: PathBuf) -> anyhow::Result<()> {
    match fs::symlink_metadata(&source) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            anyhow::bail!("bundle path '{}' is a symbolic link", source.display())
        }
        Ok(metadata) if metadata.is_dir() => copy_dir(&source, &dest),
        Ok(_) => anyhow::bail!("bundle path '{}' is not a directory", source.display()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => {
            Err(error).with_context(|| format!("inspect bundle directory '{}'", source.display()))
        }
    }
}

fn copy_dir(source: &Path, dest: &Path) -> anyhow::Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("inspect bundle directory '{}'", source.display()))?;
    if metadata.file_type().is_symlink() {
        anyhow::bail!("bundle path '{}' is a symbolic link", source.display());
    }
    if !metadata.is_dir() {
        anyhow::bail!("bundle path '{}' is not a directory", source.display());
    }
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let target = dest.join(file_name);
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect bundle path '{}'", path.display()))?;
        if file_type.is_symlink() {
            anyhow::bail!("bundle path '{}' is a symbolic link", path.display());
        }
        if file_type.is_dir() {
            copy_dir(&path, &target)?;
        } else if file_type.is_file() {
            copy_file(path, target)?;
        } else {
            anyhow::bail!(
                "bundle path '{}' has an unsupported file type",
                path.display()
            );
        }
    }
    Ok(())
}

fn default_bundle_label() -> String {
    let elapsed = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let sequence = BUNDLE_LABEL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    format!(
        "project-{}-{:09}-{}-{sequence}",
        elapsed.as_secs(),
        elapsed.subsec_nanos(),
        std::process::id()
    )
}

fn validate_bundle_label(label: &str) -> anyhow::Result<()> {
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
        anyhow::bail!(
            "deployment label must be one non-empty filesystem name, not a path: {label:?}"
        );
    }
    Ok(())
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum PointerState {
    Missing,
    Link(PathBuf),
}

fn pointer_state(path: &Path) -> anyhow::Result<PointerState> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            let target = fs::read_link(path)
                .with_context(|| format!("read deployment pointer '{}'", path.display()))?;
            Ok(PointerState::Link(target))
        }
        Ok(_) => anyhow::bail!(
            "deployment pointer '{}' exists and is not a symbolic link",
            path.display()
        ),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(PointerState::Missing),
        Err(error) => Err(error).with_context(|| format!("inspect '{}'", path.display())),
    }
}

fn validate_pointer_slot(path: &Path) -> anyhow::Result<()> {
    pointer_state(path).map(|_| ())
}

fn read_link_target(path: &Path) -> anyhow::Result<Option<PathBuf>> {
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

fn resolve_existing_bundle_link(
    bundles_dir: &Path,
    link: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(target) = read_link_target(link)? else {
        return Ok(None);
    };
    let target = match target.canonicalize() {
        Ok(target) => target,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => {
            return Err(error)
                .with_context(|| format!("resolve deployment pointer '{}'", link.display()));
        }
    };
    let bundle_store = bundles_dir.canonicalize().with_context(|| {
        format!(
            "resolve deployment bundle store '{}'",
            bundles_dir.display()
        )
    })?;
    if !target.is_dir() || target.parent() != Some(bundle_store.as_path()) {
        anyhow::bail!(
            "deployment pointer '{}' resolves outside deployment bundle store '{}': {}",
            link.display(),
            bundle_store.display(),
            target.display()
        );
    }
    Ok(Some(target))
}

fn scratch_path(path: &Path, role: &str) -> PathBuf {
    let sequence = BUNDLE_LABEL_SEQUENCE.fetch_add(1, Ordering::Relaxed);
    let name = path
        .file_name()
        .and_then(std::ffi::OsStr::to_str)
        .unwrap_or("deployment-pointer");
    path.with_file_name(format!(".{name}.{role}-{}-{sequence}", std::process::id()))
}

fn create_directory_symlink(target: &Path, link: &Path) -> std::io::Result<()> {
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)
    }
}

fn replace_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    validate_pointer_slot(link)?;
    let staged = scratch_path(link, "new");
    create_directory_symlink(target, &staged)
        .with_context(|| format!("stage deployment pointer '{}'", link.display()))?;

    #[cfg(unix)]
    {
        if let Err(error) = fs::rename(&staged, link) {
            let _ = fs::remove_file(&staged);
            return Err(error)
                .with_context(|| format!("publish deployment pointer '{}'", link.display()));
        }
    }

    #[cfg(windows)]
    {
        let backup = scratch_path(link, "old");
        let had_link = matches!(pointer_state(link)?, PointerState::Link(_));
        if had_link {
            if let Err(error) = fs::rename(link, &backup) {
                let _ = fs::remove_file(&staged);
                return Err(error)
                    .with_context(|| format!("backup deployment pointer '{}'", link.display()));
            }
        }
        if let Err(error) = fs::rename(&staged, link) {
            let restore = if had_link {
                fs::rename(&backup, link)
            } else {
                Ok(())
            };
            let _ = fs::remove_file(&staged);
            if let Err(restore_error) = restore {
                return Err(error).context(format!(
                    "publish deployment pointer '{}' and restore its backup: {restore_error}",
                    link.display()
                ));
            }
            return Err(error)
                .with_context(|| format!("publish deployment pointer '{}'", link.display()));
        }
        if had_link {
            fs::remove_file(&backup).with_context(|| {
                format!("remove deployment pointer backup '{}'", backup.display())
            })?;
        }
    }
    Ok(())
}

fn set_pointer_state(link: &Path, state: &PointerState) -> anyhow::Result<()> {
    match state {
        PointerState::Missing => match pointer_state(link)? {
            PointerState::Missing => Ok(()),
            PointerState::Link(_) => fs::remove_file(link)
                .with_context(|| format!("remove deployment pointer '{}'", link.display())),
        },
        PointerState::Link(target) => replace_symlink(link, target),
    }
}

fn restore_pointer_pair(
    current_link: &Path,
    current_state: &PointerState,
    previous_link: &Path,
    previous_state: &PointerState,
) -> anyhow::Result<()> {
    let current_result = set_pointer_state(current_link, current_state);
    let previous_result = set_pointer_state(previous_link, previous_state);
    match (current_result, previous_result) {
        (Ok(()), Ok(())) => Ok(()),
        (Err(error), Ok(())) | (Ok(()), Err(error)) => Err(error),
        (Err(current_error), Err(previous_error)) => Err(current_error.context(format!(
            "also failed to restore previous deployment pointer: {previous_error:#}"
        ))),
    }
}

fn transition_pointer_pair(
    current_link: &Path,
    new_current: &PointerState,
    previous_link: &Path,
    new_previous: &PointerState,
) -> anyhow::Result<(PointerState, PointerState)> {
    let old_current = pointer_state(current_link)?;
    let old_previous = pointer_state(previous_link)?;
    set_pointer_state(current_link, new_current)?;
    if let Err(error) = set_pointer_state(previous_link, new_previous) {
        if let Err(restore_error) =
            restore_pointer_pair(current_link, &old_current, previous_link, &old_previous)
        {
            return Err(error.context(format!(
                "also failed to restore deployment pointers: {restore_error:#}"
            )));
        }
        return Err(error);
    }
    Ok((old_current, old_previous))
}

fn update_deployment_pointers(
    current_link: &Path,
    previous_link: &Path,
    new_current: &Path,
    old_current: Option<&PathBuf>,
) -> anyhow::Result<(PointerState, PointerState)> {
    let new_current = PointerState::Link(
        new_current
            .canonicalize()
            .with_context(|| format!("resolve deployment target '{}'", new_current.display()))?,
    );
    let new_previous = match old_current {
        Some(old_current) => PointerState::Link(old_current.clone()),
        None => PointerState::Missing,
    };
    transition_pointer_pair(current_link, &new_current, previous_link, &new_previous)
}

fn swap_deployment_pointers(
    current_link: &Path,
    previous_link: &Path,
    current_target: &Path,
    previous_target: &Path,
) -> anyhow::Result<()> {
    let new_current = PointerState::Link(previous_target.to_path_buf());
    let new_previous = PointerState::Link(current_target.to_path_buf());
    transition_pointer_pair(current_link, &new_current, previous_link, &new_previous).map(|_| ())
}

#[cfg(test)]
fn update_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    let target = target
        .canonicalize()
        .with_context(|| format!("resolve deployment target '{}'", target.display()))?;
    replace_symlink(link, &target)
}

fn prune_bundles(bundles_dir: &Path, keep: &[PathBuf]) -> anyhow::Result<()> {
    if !bundles_dir.is_dir() {
        return Ok(());
    }
    let keep_set = keep
        .iter()
        .filter_map(|path| path.canonicalize().ok())
        .collect::<HashSet<_>>();
    for entry in fs::read_dir(bundles_dir)? {
        let entry = entry?;
        if !entry.file_type()?.is_dir() {
            continue;
        }
        let path = entry.path();
        let canonical = path.canonicalize().unwrap_or(path.clone());
        if keep_set.contains(&canonical) {
            continue;
        }
        fs::remove_dir_all(&path)?;
    }
    Ok(())
}

fn bundle_targets(current: &Path, previous: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut targets = vec![current.to_path_buf()];
    if let Some(previous) = previous {
        targets.push(previous.clone());
    }
    targets
}
