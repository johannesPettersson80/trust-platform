fn load_runtime(
    project: Option<PathBuf>,
    config: Option<PathBuf>,
    runtime_root: Option<PathBuf>,
) -> anyhow::Result<LoadedRuntime> {
    let ide_shell_mode = project.is_none() && config.is_none();

    if let Some(project_path) = project {
        let bundle = RuntimeBundle::load(&project_path)?;
        let sources = if bundle.root.join("src").is_dir() {
            load_project_bundle_sources(bundle.root.as_path())?
        } else {
            SourceRegistry::new(Vec::new())
        };
        // A project runs the program image it was built into for this computer (specification 44
        // §8.8, NX-15).
        let runtime = trust_runtime::CompiledRuntime::load_deployed(
            bundle.image.clone(),
            bundle.runtime.resource_name.clone(),
            bundle.runtime.max_statements_per_scan,
        )?;
        return Ok(LoadedRuntime {
            bundle: Some(bundle),
            runtime,
            sources,
            ide_shell_mode,
        });
    }

    if let Some(config_path) = config {
        let runtime_root = runtime_root.unwrap_or_else(|| {
            config_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let sources = load_sources(&runtime_root)?;
        let runtime = build_sources(&sources)?;
        return Ok(LoadedRuntime {
            bundle: None,
            runtime,
            sources,
            ide_shell_mode,
        });
    }

    let runtime_root = runtime_root
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."));
    let bootstrap_source = SourceFile {
        id: 0,
        path: runtime_root.join("__ide_bootstrap__.st"),
        text: "PROGRAM Main\nEND_PROGRAM\n".to_string(),
    };
    let sources = SourceRegistry::new(vec![bootstrap_source]);
    let runtime = build_sources(&sources)?;

    Ok(LoadedRuntime {
        bundle: None,
        runtime,
        sources,
        ide_shell_mode,
    })
}

/// The program of `sources`, built for this computer with the toolchain, installed on the first
/// build that needs it (specification 44 §8.8, §4.6): a launch without a project builds the sources
/// it is given, in a temporary folder.
fn build_sources(sources: &SourceRegistry) -> anyhow::Result<trust_runtime::CompiledRuntime> {
    let toolchain = crate::toolchain::build_toolchain()?;
    let target = trust_runtime::native::Target::this_cpu()
        .ok_or_else(|| anyhow::anyhow!("this computer's CPU runs no program image"))?;
    let files = sources
        .files()
        .iter()
        .map(|file| {
            trust_runtime::harness::SourceFile::with_path(
                file.path.to_string_lossy().as_ref(),
                file.text.clone(),
            )
        })
        .collect::<Vec<_>>();
    let work = std::env::temp_dir().join(format!("trust-runtime-sources-{}", std::process::id()));
    let built = trust_runtime::native::build(
        &files,
        &trust_runtime::native::Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    Ok(trust_runtime::CompiledRuntime::load(
        built?.image,
        "RESOURCE",
    )?)
}

fn load_project_bundle_sources(bundle_root: &Path) -> anyhow::Result<SourceRegistry> {
    load_source_registry(bundle_root, None)
}

fn load_source_registry(
    bundle_root: &Path,
    sources_root: Option<&Path>,
) -> anyhow::Result<SourceRegistry> {
    let files =
        trust_runtime::bundle_builder::collect_project_source_files(bundle_root, sources_root)?
            .into_iter()
            .enumerate()
            .map(|(index, source)| {
                let path = source
                    .path
                    .map(PathBuf::from)
                    .unwrap_or_else(|| bundle_root.join(format!("__source_{index}.st")));
                SourceFile {
                    id: u32::try_from(index).unwrap_or(u32::MAX),
                    path,
                    text: source.text,
                }
            })
            .collect();
    Ok(SourceRegistry::new(files))
}
