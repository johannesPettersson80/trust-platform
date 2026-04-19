pub struct PlayOptions {
    pub restart: String,
    pub verbose: bool,
    pub console: ConsoleMode,
    pub beginner: bool,
    pub simulation: bool,
    pub time_scale: u32,
    pub execution_backend: Option<crate::cli::ExecutionBackendArg>,
}

pub fn run_default(verbose: bool) -> anyhow::Result<()> {
    let default_options = PlayOptions {
        restart: "cold".to_string(),
        verbose,
        console: ConsoleMode::Auto,
        beginner: false,
        simulation: false,
        time_scale: 1,
        execution_backend: None,
    };
    match detect_bundle_path(None) {
        Ok(path) => run_play(Some(path), default_options),
        Err(_) => {
            if std::io::stdin().is_terminal() {
                setup::run_setup_default()
            } else {
                run_play(
                    None,
                    PlayOptions {
                        console: ConsoleMode::Disabled,
                        ..default_options
                    },
                )
            }
        }
    }
}

pub fn run_play(project: Option<PathBuf>, options: PlayOptions) -> anyhow::Result<()> {
    let mut created = false;
    let project_path = match project {
        Some(path) => {
            if should_auto_create(&path)? {
                created = true;
                wizard::create_bundle_auto(Some(path))?
            } else {
                path
            }
        }
        None => match detect_bundle_path(None).map_err(anyhow::Error::from) {
            Ok(path) => {
                if should_auto_create(&path)? {
                    created = true;
                    wizard::create_bundle_auto(Some(path))?
                } else {
                    path
                }
            }
            Err(_) => {
                created = true;
                wizard::create_bundle_auto(None)?
            }
        },
    };
    if created {
        println!(
            "{}",
            style::accent("Welcome to trueST! Creating your first PLC project...")
        );
        println!(
            "{}",
            style::success(format!(
                "Created project folder: {}",
                project_path.display()
            ))
        );
        println!("What’s next: open http://localhost:8080 to monitor this PLC.");
    }
    run_runtime(
        Some(project_path),
        None,
        None,
        options.restart,
        options.verbose,
        true,
        options.console,
        options.beginner,
        options.simulation,
        options.time_scale,
        options.execution_backend,
    )
}

pub fn run_validate(bundle: PathBuf, ci: bool) -> anyhow::Result<()> {
    let payload = validate_json_payload(bundle)?;
    if ci {
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    println!("{}", style::success("Project ok"));
    Ok(())
}

pub(crate) fn validate_json_payload(bundle: PathBuf) -> anyhow::Result<serde_json::Value> {
    let bundle = RuntimeBundle::load(&bundle)?;
    let _tls_materials = load_tls_materials(&bundle.runtime.tls, Some(bundle.root.as_path()))?;
    let control_endpoint = ControlEndpoint::parse(bundle.runtime.control_endpoint.as_str())?;
    if matches!(control_endpoint, ControlEndpoint::Tcp(_))
        && bundle.runtime.control_auth_token.is_none()
    {
        anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
    }
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    let module = BytecodeModule::decode(&bundle.bytecode)?;
    module.validate()?;
    let metadata = module.metadata()?;
    let _resource = metadata
        .resource(bundle.runtime.resource_name.as_str())
        .or_else(|| metadata.primary_resource())
        .ok_or_else(|| anyhow::anyhow!("bytecode metadata missing resource definitions"))?;
    let io_drivers = bundle
        .io
        .drivers
        .iter()
        .map(|driver| driver.name.to_string())
        .collect::<Vec<_>>();
    Ok(json!({
        "version": 1,
        "command": "validate",
        "status": "ok",
        "project": bundle.root.display().to_string(),
        "resource": bundle.runtime.resource_name.to_string(),
        "control_endpoint": bundle.runtime.control_endpoint.to_string(),
        "io_driver": io_drivers.first().cloned().unwrap_or_default(),
        "io_drivers": io_drivers,
    }))
}
