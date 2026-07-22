/// Compile bundle sources into `program.stbc`.
pub fn build_program_stbc(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<BundleBuildReport> {
    let compiled = compile_bundle_sources(bundle_root, sources_root)?;
    fs::create_dir_all(bundle_root)?;
    let program_path = bundle_root.join("program.stbc");
    fs::write(&program_path, compiled.bytes)?;

    Ok(BundleBuildReport {
        program_path,
        sources: compiled.sources,
        dependency_roots: compiled.dependency_roots,
        resolved_dependencies: compiled.resolved_dependencies,
    })
}

/// Compile bundle sources in memory without writing `program.stbc`.
pub fn check_program_stbc(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<BundleCheckReport> {
    let compiled = compile_bundle_sources(bundle_root, sources_root)?;
    Ok(BundleCheckReport {
        bytecode_size: compiled.bytes.len(),
        sources: compiled.sources,
        dependency_roots: compiled.dependency_roots,
        resolved_dependencies: compiled.resolved_dependencies,
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

struct CompiledBundleSources {
    bytes: Vec<u8>,
    sources: Vec<PathBuf>,
    dependency_roots: Vec<PathBuf>,
    resolved_dependencies: Vec<String>,
}

fn compile_bundle_sources(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<CompiledBundleSources> {
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
    Ok(CompiledBundleSources {
        bytes,
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

fn preferred_dependency_sources_root(path: &Path) -> PathBuf {
    path.join("src")
}

fn collect_sources(source_roots: &[PathBuf]) -> anyhow::Result<(Vec<SourceFile>, Vec<PathBuf>)> {
    let mut source_paths = BTreeSet::new();

    for root in source_roots {
        match fs::metadata(root) {
            Ok(metadata) if metadata.is_dir() => {
                collect_source_paths(root, &mut source_paths)?;
            }
            Ok(_) => anyhow::bail!("source root is not a directory: {}", root.display()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => continue,
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("inspect source root {}", root.display()));
            }
        }
    }

    let mut sources = Vec::with_capacity(source_paths.len());
    let mut paths = Vec::with_capacity(source_paths.len());
    for path in source_paths {
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read source file {}", path.display()))?;
        sources.push(SourceFile::with_path(
            path.to_string_lossy().to_string(),
            text,
        ));
        paths.push(path);
    }
    Ok((sources, paths))
}

fn collect_source_paths(root: &Path, paths: &mut BTreeSet<PathBuf>) -> anyhow::Result<()> {
    let mut entries = fs::read_dir(root)
        .with_context(|| format!("read source directory {}", root.display()))?
        .collect::<Result<Vec<_>, _>>()
        .with_context(|| format!("enumerate source directory {}", root.display()))?;
    entries.sort_by_key(std::fs::DirEntry::file_name);

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("inspect source path {}", path.display()))?;
        if file_type.is_dir() {
            collect_source_paths(&path, paths)?;
            continue;
        }
        if !file_type.is_file() {
            continue;
        }
        let supported = path
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_some_and(|extension| {
                extension.eq_ignore_ascii_case("st") || extension.eq_ignore_ascii_case("pou")
            });
        if supported {
            paths.insert(
                path.canonicalize()
                    .with_context(|| format!("resolve source file {}", path.display()))?,
            );
        }
    }
    Ok(())
}

fn canonicalize_or_self(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| path.to_path_buf())
}
