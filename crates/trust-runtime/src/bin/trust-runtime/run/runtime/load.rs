fn load_runtime(
    project: Option<PathBuf>,
    config: Option<PathBuf>,
    runtime_root: Option<PathBuf>,
) -> anyhow::Result<LoadedRuntime> {
    let ide_shell_mode = project.is_none() && config.is_none();

    if let Some(project_path) = project {
        let bundle = RuntimeBundle::load(&project_path)?;
        let sources = load_project_bundle_sources(bundle.root.as_path())?;
        let runtime = compile_runtime_from_sources(&sources)?;
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
        let runtime = compile_runtime_from_sources(&sources)?;
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
    let runtime = compile_runtime_from_sources(&sources)?;

    Ok(LoadedRuntime {
        bundle: None,
        runtime,
        sources,
        ide_shell_mode,
    })
}

fn compile_runtime_from_sources(sources: &SourceRegistry) -> anyhow::Result<Runtime> {
    let session = CompileSession::from_sources(
        sources
            .files()
            .iter()
            .map(|file| {
                trust_runtime::harness::SourceFile::with_path(
                    file.path.to_string_lossy().as_ref(),
                    file.text.clone(),
                )
            })
            .collect(),
    );
    let mut runtime = session.build_runtime()?;
    let bytecode = session.build_bytecode_bytes()?;
    runtime
        .apply_bytecode_bytes(&bytecode, None)
        .map_err(anyhow::Error::from)?;
    Ok(runtime)
}

fn load_project_bundle_sources(bundle_root: &Path) -> anyhow::Result<SourceRegistry> {
    let files = trust_runtime::bundle_builder::collect_project_source_files(bundle_root, None)?
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
