pub struct PlayOptions {
    pub restart: String,
    pub verbose: bool,
    pub console: ConsoleMode,
    pub beginner: bool,
    pub simulation: bool,
    pub time_scale: u32,
    /// `--paused`: start paused, before the first scan, until `resume`.
    pub paused: bool,
}

pub fn run_default(verbose: bool) -> anyhow::Result<()> {
    let default_options = PlayOptions {
        restart: "cold".to_string(),
        verbose,
        console: ConsoleMode::Auto,
        beginner: false,
        simulation: false,
        time_scale: 1,
        paused: false,
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
    validate_runtime_launch_options(&options.restart, options.time_scale)?;
    let (project_path, created) = match plan_play_project(project, || detect_bundle_path(None))? {
        PlayProjectPlan::Use(path) => (path, false),
        PlayProjectPlan::Create(path) => (wizard::create_bundle_auto(path)?, true),
    };
    if created {
        outln!(
            "{}",
            style::accent("Welcome to trueST! Creating your first PLC project...")
        );
        outln!(
            "{}",
            style::success(format!(
                "Created project folder: {}",
                project_path.display()
            ))
        );
        outln!("What’s next: open http://localhost:8080 to monitor this PLC.");
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
        options.paused,
    )
}

pub fn run_validate(bundle: PathBuf, ci: bool) -> anyhow::Result<()> {
    let payload = validate_json_payload(bundle)?;
    if ci {
        outln!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    // Each composed overlap, as the start warns of it (specification 11 §6.5), and then the
    // verdict: the project still starts.
    for warning in payload["warnings"].as_array().into_iter().flatten() {
        outln!("{}", style::warning(overlap_warning(warning)));
    }
    outln!("{}", style::success("Project ok"));
    Ok(())
}

/// The line of a composed overlap's warning, from the event's data.
fn overlap_warning(data: &serde_json::Value) -> String {
    let (earlier, later) = (data["earlier"].as_str().unwrap_or(""), data["later"].as_str().unwrap_or(""));
    let (first, last) = (data["first_byte"].as_u64().unwrap_or(0), data["last_byte"].as_u64().unwrap_or(0));
    let bytes = if first == last {
        format!("input byte {first}")
    } else {
        format!("input bytes {first} to {last}")
    };
    format!(
        "warning: {earlier} and {later} both fill {bytes}; {later}, the later, overwrites what {earlier} read there at every cycle"
    )
}

pub(crate) fn validate_json_payload(bundle: PathBuf) -> anyhow::Result<serde_json::Value> {
    let bundle = RuntimeBundle::load(&bundle)?;
    let _tls_materials = load_tls_materials(&bundle.runtime.tls, Some(bundle.root.as_path()))?;
    // An omitted endpoint is the platform default, which generates its own token when it is TCP.
    if let Some(configured) = bundle.runtime.control_endpoint.as_deref() {
        let control_endpoint = ControlEndpoint::parse(configured)?;
        if matches!(control_endpoint, ControlEndpoint::Tcp(_))
            && bundle.runtime.control_auth_token.is_none()
        {
            anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
        }
    }
    let registry = IoDriverRegistry::default_registry();
    for driver in &bundle.io.drivers {
        if !driver.enabled {
            continue;
        }
        registry
            .validate(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?;
    }
    // The program image is admitted as the computer admits it at its start (specification 44
    // §10.2, §8.8).
    trust_runtime::native::admit_on_this_computer(&bundle.image)?;
    trust_runtime::native::check_resource(&bundle.image, &bundle.runtime.resource_name)?;
    // An MQTT driver's tag mappings are part of its configuration, which the start resolves
    // against the program before it builds the driver (specification 32 §5): `validate` validates
    // the I/O-driver configurations as the start does (specification 11), and finds the composed
    // drivers that would fill the same input bytes, of which the start warns (§6.5).
    let overlaps = composed_input_overlaps(&bundle)?;
    // Launch's own identify check (spec 11, "Identify", load): a project that `validate` accepts is
    // a project that `run` accepts.
    check_bundle_identify_output(&bundle)?;
    let io_drivers = enabled_io_driver_names(&bundle);
    Ok(json!({
        "version": 1,
        "command": "validate",
        "status": "ok",
        "project": bundle.root.display().to_string(),
        "resource": bundle.runtime.resource_name.to_string(),
        "control_endpoint": project_control_endpoint_text(
            &bundle.root,
            bundle.runtime.control_endpoint.as_deref(),
        ),
        "io_driver": io_drivers.first().cloned().unwrap_or_default(),
        "io_drivers": io_drivers,
        "warnings": overlaps.iter().map(|overlap| overlap.event_data()).collect::<Vec<_>>(),
    }))
}

/// The composed drivers of the bundle's `io.toml` that would fill the same input bytes, from their
/// parameters, as the start warns of them (`CompiledRuntime::io_config_input_overlaps`,
/// specification 11 §6.5). It resolves each enabled MQTT driver's tag mappings against the
/// bundle's program as the start does, so that a tag the start refuses fails validation with the
/// start's error. A project with one enabled driver and no tag mappings loads no program.
fn composed_input_overlaps(
    bundle: &RuntimeBundle,
) -> anyhow::Result<Vec<trust_runtime::io::InputOverlap>> {
    let enabled: Vec<_> = bundle
        .io
        .drivers
        .iter()
        .filter(|driver| driver.enabled)
        .collect();
    let mapped = enabled.iter().any(|driver| {
        trust_runtime::io::takes_tag_mappings(driver.name.as_str())
            && driver.params.get("mappings").is_some()
    });
    if enabled.len() < 2 && !mapped {
        return Ok(Vec::new());
    }
    let mut program = trust_runtime::CompiledRuntime::load(
        bundle.image.clone(),
        bundle.runtime.resource_name.clone(),
    )?;
    Ok(program.io_config_input_overlaps(&bundle.io)?)
}

/// Why a project's runtime cannot run its program: runtime.toml names a resource the program's
/// configuration does not declare.
pub(crate) fn resource_mismatch(name: &str, declared: &[&str]) -> String {
    let declared = match declared {
        [] => "no resource".to_string(),
        [one] => format!("resource '{one}'"),
        many => format!(
            "the resources {}",
            many.iter()
                .map(|name| format!("'{name}'"))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    };
    format!(
        "runtime.toml names resource '{name}', but the program's configuration declares {declared}; use the same name in both"
    )
}
