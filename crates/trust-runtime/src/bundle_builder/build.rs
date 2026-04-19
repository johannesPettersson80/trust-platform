/// Compile bundle sources into `program.stbc`.
pub fn build_program_stbc(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<BundleBuildReport> {
    let sources_root = resolve_sources_root(bundle_root, sources_root)?;

    let dependencies = resolve_local_dependencies(bundle_root)?;
    let mut source_roots = vec![sources_root.clone()];
    for dependency in &dependencies {
        source_roots.push(preferred_dependency_sources_root(&dependency.path));
    }

    let (sources, source_paths) = collect_sources(&source_roots)?;
    if sources.is_empty() {
        anyhow::bail!(
            "no source files found in {} (expected .st/.pou files)",
            sources_root.display()
        );
    }

    let session = CompileSession::from_sources(sources);
    let bytes = session.build_bytecode_bytes()?;
    fs::create_dir_all(bundle_root)?;
    let program_path = bundle_root.join("program.stbc");
    fs::write(&program_path, bytes)?;

    Ok(BundleBuildReport {
        program_path,
        sources: source_paths,
        dependency_roots: dependencies
            .iter()
            .map(|dependency| dependency.path.clone())
            .collect(),
        resolved_dependencies: dependencies
            .iter()
            .map(|dependency| dependency.name.clone())
            .collect(),
    })
}

/// Collect compile sources for a project, including any local package dependencies.
pub fn collect_project_source_files(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<Vec<SourceFile>> {
    let sources_root = resolve_sources_root(bundle_root, sources_root)?;
    let dependencies = resolve_local_dependencies(bundle_root)?;

    let mut source_roots = vec![sources_root];
    for dependency in &dependencies {
        source_roots.push(preferred_dependency_sources_root(&dependency.path));
    }

    let (sources, _) = collect_sources(&source_roots)?;
    Ok(sources)
}

/// Inspect project sources/dependencies without building bytecode.
pub fn inspect_project_layout(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<ProjectInspectionReport> {
    let sources_root = resolve_sources_root(bundle_root, sources_root)?;
    let dependencies = resolve_local_dependencies(bundle_root)?;

    let mut source_roots = vec![sources_root.clone()];
    for dependency in &dependencies {
        source_roots.push(preferred_dependency_sources_root(&dependency.path));
    }

    let (_, source_paths) = collect_sources(&source_roots)?;
    Ok(ProjectInspectionReport {
        sources_root,
        manifest_path: find_dependency_manifest(bundle_root),
        sources: source_paths,
        dependency_roots: dependencies
            .iter()
            .map(|dependency| dependency.path.clone())
            .collect(),
        resolved_dependencies: dependencies
            .iter()
            .map(|dependency| dependency.name.clone())
            .collect(),
    })
}

/// Resolve the effective project source root for bundle operations.
///
/// Behavior:
/// - if `sources_root` is provided and relative, it is resolved relative to `bundle_root`
/// - default search uses `src/`
pub fn resolve_sources_root(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<PathBuf> {
    if let Some(override_root) = sources_root {
        let resolved = if override_root.is_absolute() {
            override_root.to_path_buf()
        } else {
            bundle_root.join(override_root)
        };
        let resolved = canonicalize_or_self(&resolved);
        if !resolved.is_dir() {
            anyhow::bail!("sources directory not found: {}", resolved.display());
        }
        return Ok(resolved);
    }

    let src_root = bundle_root.join("src");
    if src_root.is_dir() {
        return Ok(canonicalize_or_self(&src_root));
    }

    anyhow::bail!(
        "invalid project folder '{}': missing src/ directory",
        bundle_root.display()
    );
}

fn preferred_dependency_sources_root(path: &Path) -> PathBuf {
    path.join("src")
}

fn collect_sources(source_roots: &[PathBuf]) -> anyhow::Result<(Vec<SourceFile>, Vec<PathBuf>)> {
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    let mut seen = BTreeSet::new();
    let mut source_map = BTreeMap::new();

    for root in source_roots {
        if !root.is_dir() {
            continue;
        }
        for pattern in patterns {
            for entry in glob::glob(&format!("{}/{}", root.display(), pattern))? {
                let path = entry?;
                if !path.is_file() {
                    continue;
                }
                let resolved = canonicalize_or_self(&path);
                let path_text = resolved.to_string_lossy().to_string();
                if !seen.insert(path_text.clone()) {
                    continue;
                }
                let text = fs::read_to_string(&resolved)?;
                source_map.insert(path_text, text);
            }
        }
    }

    let mut sources = Vec::with_capacity(source_map.len());
    let mut paths = Vec::with_capacity(source_map.len());
    for (path, text) in source_map {
        paths.push(PathBuf::from(&path));
        sources.push(SourceFile::with_path(path, text));
    }
    Ok((sources, paths))
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
