pub fn run_setup(options: SetupCommandOptions) -> anyhow::Result<()> {
    let SetupCommandOptions {
        mode,
        access,
        project,
        bind,
        port,
        token_ttl_minutes,
        dry_run,
        driver,
        backend,
        path,
        force,
    } = options;
    let system_setup_requested = driver.is_some() || backend.is_some() || path.is_some() || force;
    if system_setup_requested {
        validate_system_setup_flag_mix(
            mode,
            access,
            project.as_ref(),
            bind.as_ref(),
            port,
            token_ttl_minutes,
            dry_run,
        )?;
        return run_system_setup(driver, backend, path, force);
    }
    if let Some(mode) = mode {
        return run_setup_mode(
            mode,
            access,
            project,
            bind,
            port,
            token_ttl_minutes,
            dry_run,
        );
    }
    if !std::io::stdin().is_terminal() {
        anyhow::bail!(
            "setup requires an interactive terminal, or explicit mode (e.g. `trust-runtime setup --mode cancel`)"
        );
    }
    println!(
        "{}",
        style::accent("Welcome to trueST! Let’s set up your first PLC project.")
    );
    println!("Setup options:");
    println!("  1) Open browser setup");
    println!("  2) Start CLI setup");
    println!("  3) Cancel setup");
    let choice = prompt::prompt_string("Select option", "1")?;
    match choice.trim() {
        "1" => run_browser_setup_interactive(),
        "2" => run_cli_setup_interactive(),
        "3" => {
            print_cancel_message();
            Ok(())
        }
        _ => anyhow::bail!(
            "Invalid option. Expected 1, 2, or 3. Tip: run trust-runtime setup again."
        ),
    }
}

pub fn run_setup_default() -> anyhow::Result<()> {
    crate::style::print_logo();
    println!(
        "{}",
        style::accent("Welcome to trueST! Let’s create your first PLC project.")
    );
    println!("If you are on another device, run: trust-runtime setup");
    run_browser_setup_auto()
}

fn run_setup_mode(
    mode: SetupModeArg,
    access: Option<SetupAccessArg>,
    project: Option<PathBuf>,
    bind: Option<String>,
    port: Option<u16>,
    token_ttl_minutes: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    match mode {
        SetupModeArg::Cancel => {
            print_cancel_message();
            Ok(())
        }
        SetupModeArg::Browser => {
            run_browser_setup_mode(
                access.unwrap_or(SetupAccessArg::Local),
                project,
                bind,
                port.unwrap_or(DEFAULT_SETUP_PORT),
                token_ttl_minutes,
                dry_run,
            )
        }
        SetupModeArg::Cli => {
            if access.is_some()
                || bind.is_some()
                || port.is_some()
                || token_ttl_minutes.is_some()
            {
                anyhow::bail!(
                    "browser-only setup options require --mode browser: --access, --bind, --port, --token-ttl-minutes"
                );
            }
            run_cli_guided_noninteractive(project, dry_run)
        }
    }
}

fn validate_system_setup_flag_mix(
    mode: Option<SetupModeArg>,
    access: Option<SetupAccessArg>,
    project: Option<&PathBuf>,
    bind: Option<&String>,
    port: Option<u16>,
    token_ttl_minutes: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    if mode.is_some()
        || project.is_some()
        || bind.is_some()
        || token_ttl_minutes.is_some()
        || access.is_some()
        || port.is_some()
        || dry_run
    {
        anyhow::bail!(
            "system setup flags (--driver/--backend/--path/--force) cannot be combined with guided setup options (--mode/--access/--project/--bind/--port/--token-ttl-minutes/--dry-run)"
        );
    }
    Ok(())
}

fn run_system_setup(
    driver: Option<String>,
    backend: Option<String>,
    path: Option<PathBuf>,
    force: bool,
) -> anyhow::Result<()> {
    let options = trust_runtime::setup::SetupOptions {
        driver: driver.map(SmolStr::new),
        backend: backend.map(SmolStr::new),
        force,
        path,
    };
    let path = trust_runtime::setup::run_setup(options)?;
    println!(
        "{}",
        style::success(format!("System I/O config written to {}", path.display()))
    );
    Ok(())
}
