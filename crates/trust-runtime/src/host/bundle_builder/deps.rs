fn resolve_local_dependencies(bundle_root: &Path) -> anyhow::Result<Vec<ResolvedDependency>> {
    let manifest = load_dependency_manifest(bundle_root).with_context(|| {
        format!(
            "failed to load dependency manifest at {}",
            bundle_root.display()
        )
    })?;
    let declared = parse_dependency_specs(bundle_root, &manifest.dependencies);

    let mut resolved = BTreeMap::new();
    let mut states = HashMap::new();
    let mut stack = Vec::new();
    for dependency in &declared {
        resolve_dependency_recursive(dependency, &mut states, &mut stack, &mut resolved)?;
    }
    Ok(resolved.into_values().collect())
}

fn resolve_dependency_recursive(
    dependency: &DependencySpec,
    states: &mut HashMap<String, DependencyVisitState>,
    stack: &mut Vec<String>,
    resolved: &mut BTreeMap<String, ResolvedDependency>,
) -> anyhow::Result<()> {
    let path = canonicalize_or_self(&dependency.path);
    if !path.is_dir() {
        anyhow::bail!(
            "dependency '{}' path does not exist: {}",
            dependency.name,
            path.display()
        );
    }
    let dependency_src = path.join("src");
    if !dependency_src.is_dir() {
        anyhow::bail!(
            "dependency '{}' missing src/ directory: {}",
            dependency.name,
            dependency_src.display()
        );
    }

    if let Some(existing) = resolved.get(&dependency.name) {
        ensure_dependency_version(
            dependency.name.as_str(),
            dependency.version.as_deref(),
            existing.version.as_deref(),
        )?;
        return Ok(());
    }

    match states.get(dependency.name.as_str()).copied() {
        Some(DependencyVisitState::Visiting) => {
            let mut cycle = stack.clone();
            cycle.push(dependency.name.clone());
            anyhow::bail!("cyclic dependency detected: {}", cycle.join(" -> "));
        }
        Some(DependencyVisitState::Done) => return Ok(()),
        None => {}
    }

    states.insert(dependency.name.clone(), DependencyVisitState::Visiting);
    stack.push(dependency.name.clone());

    let manifest = load_dependency_manifest(&path).with_context(|| {
        format!(
            "failed to load dependency manifest for '{}' ({})",
            dependency.name,
            path.display()
        )
    })?;
    ensure_dependency_version(
        dependency.name.as_str(),
        dependency.version.as_deref(),
        manifest.package.version.as_deref(),
    )?;

    let nested = parse_dependency_specs(&path, &manifest.dependencies);
    for nested_dependency in &nested {
        resolve_dependency_recursive(nested_dependency, states, stack, resolved)?;
    }

    resolved.insert(
        dependency.name.clone(),
        ResolvedDependency {
            name: dependency.name.clone(),
            path,
            version: manifest.package.version,
        },
    );
    let _ = stack.pop();
    states.insert(dependency.name.clone(), DependencyVisitState::Done);
    Ok(())
}

fn ensure_dependency_version(
    name: &str,
    required: Option<&str>,
    actual: Option<&str>,
) -> anyhow::Result<()> {
    if let Some(required) = required {
        if actual != Some(required) {
            let resolved = actual.unwrap_or("unspecified");
            anyhow::bail!(
                "dependency '{}' requested version {}, but resolved package version is {}",
                name,
                required,
                resolved
            );
        }
    }
    Ok(())
}

fn parse_dependency_specs(
    root: &Path,
    entries: &BTreeMap<String, ManifestDependencyEntry>,
) -> Vec<DependencySpec> {
    entries
        .iter()
        .map(|(name, entry)| DependencySpec {
            name: name.clone(),
            path: resolve_path(root, entry.path()),
            version: entry.version(),
        })
        .collect()
}

fn load_dependency_manifest(root: &Path) -> anyhow::Result<DependencyManifestFile> {
    let Some(path) = find_dependency_manifest(root) else {
        return Ok(DependencyManifestFile::default());
    };
    let contents = fs::read_to_string(&path)
        .with_context(|| format!("failed to read dependency manifest {}", path.display()))?;
    let parsed = toml::from_str(&contents)
        .with_context(|| format!("failed to parse dependency manifest {}", path.display()))?;
    Ok(parsed)
}

fn find_dependency_manifest(root: &Path) -> Option<PathBuf> {
    DEPENDENCY_MANIFEST_FILES
        .iter()
        .map(|name| root.join(name))
        .find(|path| path.is_file())
}

fn resolve_path(root: &Path, value: &str) -> PathBuf {
    let path = PathBuf::from(value);
    if path.is_absolute() {
        path
    } else {
        root.join(path)
    }
}
