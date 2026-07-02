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
    let secs = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    format!("project-{secs}")
}

fn read_link_target(path: &Path) -> Option<PathBuf> {
    fs::read_link(path).ok()
}

fn update_symlink(link: &Path, target: &Path) -> anyhow::Result<()> {
    if link.exists() {
        fs::remove_file(link)?;
    }
    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(target, link)?;
    }
    #[cfg(windows)]
    {
        std::os::windows::fs::symlink_dir(target, link)?;
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
        if !entry.path().is_dir() {
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
