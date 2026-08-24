fn format_retain_mode(mode: trust_runtime::watchdog::RetainMode) -> &'static str {
    match mode {
        trust_runtime::watchdog::RetainMode::None => "none",
        trust_runtime::watchdog::RetainMode::File => "file",
    }
}

fn format_web_url(listen: &str, tls: bool) -> String {
    let host = listen.split(':').next().unwrap_or("localhost");
    let port = listen.rsplit(':').next().unwrap_or("8080");
    let host = if host == "0.0.0.0" { "localhost" } else { host };
    let scheme = if tls { "https" } else { "http" };
    format!("{scheme}://{host}:{port}")
}

fn simulation_warning_message(enabled: bool, time_scale: u32) -> Option<String> {
    if !enabled {
        return None;
    }
    Some(format!(
        "Simulation mode active (time scale x{}). Not for live hardware.",
        time_scale.max(1)
    ))
}

fn validate_runtime_launch_options(restart: &str, time_scale: u32) -> anyhow::Result<RestartMode> {
    let restart_mode = parse_restart_mode(restart)?;
    if time_scale == 0 {
        anyhow::bail!("--time-scale must be >= 1");
    }
    Ok(restart_mode)
}

fn enabled_io_driver_names(bundle: &RuntimeBundle) -> Vec<String> {
    bundle
        .io
        .drivers
        .iter()
        .filter(|driver| driver.enabled)
        .map(|driver| driver.name.to_string())
        .collect()
}

fn should_auto_create(path: &Path) -> anyhow::Result<bool> {
    if !path.exists() {
        return Ok(true);
    }
    if !path.is_dir() {
        anyhow::bail!("project folder is not a directory: {}", path.display());
    }
    let runtime_toml = path.join("runtime.toml");
    let program_stbc = path.join("program.stbc");
    Ok(!runtime_toml.is_file() || !program_stbc.is_file())
}

#[derive(Debug, PartialEq, Eq)]
enum PlayProjectPlan {
    Use(PathBuf),
    Create(Option<PathBuf>),
}

fn plan_play_project(
    project: Option<PathBuf>,
    detect: impl FnOnce() -> Result<PathBuf, trust_runtime::error::RuntimeError>,
) -> anyhow::Result<PlayProjectPlan> {
    let candidate = match project {
        Some(path) => path,
        None => match detect() {
            Ok(path) => path,
            Err(error) if is_bundle_not_found(&error) => {
                return Ok(PlayProjectPlan::Create(None));
            }
            Err(error) => return Err(error.into()),
        },
    };
    if should_auto_create(&candidate)? {
        Ok(PlayProjectPlan::Create(Some(candidate)))
    } else {
        Ok(PlayProjectPlan::Use(candidate))
    }
}
