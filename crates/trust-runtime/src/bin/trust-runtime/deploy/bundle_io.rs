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

fn copy_bundle(source: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    copy_file(source.join("runtime.toml"), dest.join("runtime.toml"))?;
    if source.join("io.toml").is_file() {
        copy_file(source.join("io.toml"), dest.join("io.toml"))?;
    }
    if source.join("simulation.toml").is_file() {
        copy_file(source.join("simulation.toml"), dest.join("simulation.toml"))?;
    }
    copy_file(source.join("program.stbc"), dest.join("program.stbc"))?;

    let sources = source.join("src");
    if sources.is_dir() {
        copy_dir(&sources, &dest.join("src"))?;
    }
    Ok(())
}

fn copy_file(source: PathBuf, dest: PathBuf) -> anyhow::Result<()> {
    if !source.is_file() {
        anyhow::bail!("missing file {}", source.display());
    }
    fs::copy(&source, &dest)?;
    Ok(())
}

fn copy_dir(source: &Path, dest: &Path) -> anyhow::Result<()> {
    fs::create_dir_all(dest)?;
    for entry in fs::read_dir(source)? {
        let entry = entry?;
        let path = entry.path();
        let file_name = entry.file_name();
        let target = dest.join(file_name);
        if path.is_dir() {
            copy_dir(&path, &target)?;
        } else {
            fs::copy(&path, &target)?;
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

fn read_link_target(path: &Path) -> Option<PathBuf> {
    let target = fs::read_link(path).ok()?;
    if target.is_absolute() {
        return Some(target);
    }
    let parent = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    Some(parent.join(target))
}

fn resolve_existing_bundle_link(
    bundles_dir: &Path,
    link: &Path,
) -> anyhow::Result<Option<PathBuf>> {
    let Some(target) = read_link_target(link) else {
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

fn update_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    let target = target
        .canonicalize()
        .with_context(|| format!("resolve deployment target '{}'", target.display()))?;
    match fs::symlink_metadata(link) {
        Ok(metadata) if !metadata.file_type().is_symlink() => {
            anyhow::bail!(
                "deployment pointer '{}' exists and is not a symbolic link",
                link.display()
            );
        }
        Ok(_) => fs::remove_file(link)
            .with_context(|| format!("remove deployment pointer '{}'", link.display()))?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => return Err(error).with_context(|| format!("inspect '{}'", link.display())),
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&target, link)?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(&target, link)?;
    }
    Ok(())
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
