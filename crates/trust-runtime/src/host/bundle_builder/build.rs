/// Builds the project's program image for `target` with `toolchain` (specification 44 §4.1, §8.8).
/// The image for this computer, `for_this_computer`, is the project's program, `program.tpi` with
/// its deployment record `deployment-record.bin` (NX-15); an image for another target goes into the
/// build folder as `program.<triple>.tpi` with `deployment-record.<triple>.bin`. The project's
/// `[build] debug` chooses a debug or a release image (§13.2), its `[project] vendor_profile` the
/// conversion rules (specification 07 §2.10), the image is signed with the
/// project's key or the user's own (§5.11) before its record names its bytes (§10.1), and
/// `build/build.json` gets the report. The generated crate stays in the build folder with
/// `emit_source`. A source set that declares OpenOT data gets its hash-bound
/// `openot-definition.json` too.
pub fn build_program_image(
    bundle_root: &Path,
    sources_root: Option<&Path>,
    target: crate::native::Target,
    toolchain: &crate::native::Toolchain,
    emit_source: bool,
    for_this_computer: bool,
) -> anyhow::Result<BundleBuildReport> {
    use crate::native::{
        build_with_conversions, conversion_profile, debug_hooks, project_key, sign, triple, Options,
    };
    let layout = inspect_project_layout(bundle_root, sources_root)?;
    let files = collect_project_sources_labelled(bundle_root, sources_root)?;
    if files.is_empty() {
        anyhow::bail!(
            "no source files found in {} (expected .st/.pou files)",
            layout.sources_root.display()
        );
    }
    let debug = debug_hooks(bundle_root)?;
    let conversions = conversion_profile(bundle_root)?;
    let build_folder = bundle_root.join("build");
    let work = build_folder.join("native").join(triple(target));
    let mut built = build_with_conversions(
        &files,
        &[],
        &Options {
            target,
            hooks: debug,
        },
        conversions,
        toolchain,
        &work,
    )?;
    check_configured_resource(bundle_root, built.resource.as_deref())?;
    let key = project_key(bundle_root)?;
    sign(&mut built.image, &key)?;
    let signer = hex::encode(key.verifying_key().as_bytes());
    let (image, record) = if for_this_computer {
        (
            bundle_root.join(crate::native::PROGRAM_IMAGE_FILE),
            bundle_root.join(crate::deployment::DEPLOYMENT_RECORD_FILE),
        )
    } else {
        (
            build_folder.join(format!("program.{}.tpi", triple(target))),
            build_folder.join(format!("deployment-record.{}.bin", triple(target))),
        )
    };
    fs::create_dir_all(&build_folder)?;
    fs::write(&image, &built.image).with_context(|| format!("write {}", image.display()))?;
    let record_bytes = crate::deployment::image_record(bundle_root, &built.image)?;
    fs::write(&record, &record_bytes).with_context(|| format!("write {}", record.display()))?;
    // The container `trust-runtime ctl install` installs, the image with its record, packed as every
    // image container is (specification 34 §5.1 and §11.2; specification 44 §4.1).
    let install = build_folder.join(format!("install.{}.bin", triple(target)));
    let container = trust_runtime_services::installation::encode_image(
        trust_runtime_services::installation::FIRST_SEQUENCE,
        &built.image,
        &record_bytes,
    )
    .map_err(|error| anyhow::anyhow!("pack {}: {error}", install.display()))?;
    fs::write(&install, container).with_context(|| format!("write {}", install.display()))?;
    if !emit_source {
        let _ = fs::remove_file(work.join("module.rs"));
    }
    let openot_definition_path = write_openot_definition(bundle_root, &files)?;
    let figure = built.figure;
    let source_paths = |paths: &[PathBuf]| {
        paths
            .iter()
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
    };
    let json = serde_json::json!({
        "version": 1,
        "command": "build",
        "status": "ok",
        "project": bundle_root.display().to_string(),
        "source_count": layout.sources.len(),
        "sources": source_paths(&layout.sources),
        "dependency_roots": source_paths(&layout.dependency_roots),
        "resolved_dependencies": layout.resolved_dependencies,
        "target": triple(target),
        "debug": debug,
        "image": image.display().to_string(),
        "image_bytes": built.image.len(),
        "signer": signer,
        "record": record.display().to_string(),
        "install": install.display().to_string(),
        "memory_figure": {
            "areas": figure.areas,
            "stack": figure.stack,
            "bookkeeping": figure.bookkeeping,
            "retain_capture": figure.retain_capture,
            "total": figure.total,
            "call_level": figure.call_level,
        },
        "flash": {
            "code": figure.code,
            "rodata": figure.rodata,
            "init": figure.init,
        },
    });
    fs::write(
        build_folder.join("build.json"),
        serde_json::to_string_pretty(&json)? + "\n",
    )?;
    Ok(BundleBuildReport {
        image,
        record,
        install,
        debug,
        image_bytes: built.image.len(),
        figure,
        signer,
        openot_definition_path,
        sources: layout.sources,
        dependency_roots: layout.dependency_roots,
        resolved_dependencies: layout.resolved_dependencies,
        json,
    })
}

/// Refuses a project whose `runtime.toml` names another resource than the `RESOURCE` its program
/// declares, compared without regard to case (specification 11 §6.7.1): an image runs the one
/// resource it was built from. A program without a `CONFIGURATION`, `declared` `None`, runs under
/// the name the project gives it, and so does a project without `runtime.toml`.
fn check_configured_resource(bundle_root: &Path, declared: Option<&str>) -> anyhow::Result<()> {
    let Some(declared) = declared else {
        return Ok(());
    };
    let runtime_toml = bundle_root.join("runtime.toml");
    if !runtime_toml.is_file() {
        return Ok(());
    }
    let configured = crate::config::RuntimeConfig::load(&runtime_toml)?.resource_name;
    if configured.eq_ignore_ascii_case(declared) {
        return Ok(());
    }
    anyhow::bail!(
        "resource '{configured}' of runtime.toml is not the RESOURCE the program declares ('{declared}')"
    )
}

/// Writes the hash-bound OpenOT definition of `files` as `openot-definition.json`, when the source
/// set declares OpenOT data.
fn write_openot_definition(
    bundle_root: &Path,
    files: &[SourceFile],
) -> anyhow::Result<Option<PathBuf>> {
    let definition = match crate::openot_authoring::definition_json_from_sources(files) {
        Ok(definition) => definition,
        Err(error) if error == "no OpenOT declaration attributes found" => return Ok(None),
        Err(error) => {
            return Err(anyhow::Error::msg(error))
                .context("generate OpenOT definition from compiled source set");
        }
    };
    let path = bundle_root.join("openot-definition.json");
    let mut bytes =
        serde_json::to_vec_pretty(&definition).context("serialize compiled OpenOT definition")?;
    bytes.push(b'\n');
    fs::write(&path, bytes).with_context(|| format!("write {}", path.display()))?;
    Ok(Some(path))
}

/// Checks the project's sources without the toolchain: the build's front and its generator run for
/// this computer's CPU with the project's conversion rules, as a build for it would (specification
/// 44 §4.1; specification 07 §2.10), and nothing is written. The report names the resource the
/// program declares, which `trust-runtime check` compares with the project's `runtime.toml`.
pub fn check_program(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<BundleCheckReport> {
    let layout = inspect_project_layout(bundle_root, sources_root)?;
    let files = collect_project_sources_labelled(bundle_root, sources_root)?;
    if files.is_empty() {
        anyhow::bail!(
            "no source files found in {} (expected .st/.pou files)",
            layout.sources_root.display()
        );
    }
    let target = crate::native::Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    // A misspelled `vendor_profile` fails the check, as it fails the build
    // (docs/specs/07-standard-functions.md 2.10).
    let conversions = crate::native::conversion_profile(bundle_root)?;
    crate::native::generate_source_with_conversions(
        &files,
        &crate::native::Options {
            target,
            hooks: false,
        },
        conversions,
    )?;
    // The `RESOURCE` the program's CONFIGURATION declares, as the build reads it from the lowered
    // configuration; none without a CONFIGURATION (specification 11 §6.8.1d).
    let (lowered, _) =
        crate::harness::lower_application_and_frontend(&files, true, false, conversions)?;
    let resources = lowered
        .configuration
        .and_then(|configuration| configuration.resource_name)
        .map(|name| name.to_string())
        .into_iter()
        .collect();
    Ok(BundleCheckReport {
        sources: layout.sources,
        dependency_roots: layout.dependency_roots,
        resolved_dependencies: layout.resolved_dependencies,
        resources,
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

/// The source folders of a project's local package dependencies (`[dependencies]` in its
/// manifest, recursively), resolved as a build resolves them; none without a manifest.
pub fn dependency_source_roots(bundle_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    Ok(resolve_local_dependencies(bundle_root)?
        .iter()
        .map(|dependency| preferred_dependency_sources_root(&dependency.path))
        .collect())
}

/// The folders of a project's local package dependencies, recursively, as a build resolves them;
/// none without a manifest. A library's HMI files live under `hmi/` in its folder (spec 39, §8.7).
pub fn dependency_roots(bundle_root: &Path) -> anyhow::Result<Vec<PathBuf>> {
    Ok(resolve_local_dependencies(bundle_root)?
        .into_iter()
        .map(|dependency| dependency.path)
        .collect())
}

/// The project's source files as [`collect_project_source_files`] finds them, each labelled by its
/// path relative to the project root with `/` separators (specification 12 §7.5.1.4, which
/// specification 44 §5.3 keeps), so that a client on any computer names a file by a path that ends
/// with its label. A file outside the project root, such as a dependency's, keeps its full path.
pub fn collect_project_sources_labelled(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<Vec<SourceFile>> {
    let root = bundle_root
        .canonicalize()
        .with_context(|| format!("resolve project root {}", bundle_root.display()))?;
    let mut sources = collect_project_source_files(bundle_root, sources_root)?;
    for source in &mut sources {
        if let Some(path) = source.path.as_deref()
            && let Ok(relative) = Path::new(path).strip_prefix(&root)
        {
            let label = relative
                .components()
                .map(|part| part.as_os_str().to_string_lossy())
                .collect::<Vec<_>>()
                .join("/");
            source.path = Some(label);
        }
    }
    Ok(sources)
}

/// Inspect project sources/dependencies without building the program.
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
