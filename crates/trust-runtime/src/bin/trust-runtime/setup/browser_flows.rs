fn run_browser_setup_interactive() -> anyhow::Result<()> {
    println!("Where will you open the browser?");
    println!("  1) On this device (local GUI)");
    println!("  2) From another device (headless/SSH)");
    let choice = prompt::prompt_string("Select option", "2")?;
    let access = if matches!(choice.trim(), "2") {
        SetupAccessArg::Remote
    } else {
        SetupAccessArg::Local
    };
    let token_ttl_minutes = if matches!(access, SetupAccessArg::Remote) {
        println!("Token expiry:");
        println!("  1) 15 min (default)");
        println!("  2) 30 min");
        println!("  3) 60 min");
        println!("  4) Custom");
        let ttl = match prompt::prompt_string("Select option", "1")?.as_str() {
            "2" => 30,
            "3" => 60,
            "4" => prompt::prompt_u64("Minutes", DEFAULT_REMOTE_TOKEN_TTL_MINUTES)?,
            _ => DEFAULT_REMOTE_TOKEN_TTL_MINUTES,
        };
        Some(ttl)
    } else {
        None
    };
    let advanced = prompt::prompt_yes_no("Advanced settings?", false)?;
    let default_bind = match access {
        SetupAccessArg::Local => "127.0.0.1",
        SetupAccessArg::Remote => "0.0.0.0",
    };
    let bind = if advanced {
        Some(prompt::prompt_string("Bind address", default_bind)?)
    } else {
        None
    };
    let port = if advanced {
        setup_port_from_prompt_value(prompt::prompt_u64("Port", DEFAULT_SETUP_PORT.into())?)?
    } else {
        DEFAULT_SETUP_PORT
    };
    println!("Project folder: where runtime.toml, io.toml, src/, program.stbc live.");
    let default_bundle = default_bundle_path();
    let bundle_path = prompt::prompt_path("Project folder (runtime files)", &default_bundle)?;
    run_browser_setup_mode(
        access,
        Some(bundle_path),
        bind,
        port,
        token_ttl_minutes,
        false,
    )
}

fn run_browser_setup_mode(
    access: SetupAccessArg,
    project: Option<PathBuf>,
    bind: Option<String>,
    port: u16,
    token_ttl_minutes: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    let profile = BrowserSetupProfile::build(access, bind, port, token_ttl_minutes)?;
    let bundle_path = project.unwrap_or_else(default_bundle_path);
    let defaults = SetupDefaults::from_bundle(&bundle_path);
    if dry_run {
        println!("{}", style::accent("Setup dry run (browser mode)"));
        println!("Project: {}", bundle_path.display());
        println!(
            "Access: {}",
            match access {
                SetupAccessArg::Local => "local",
                SetupAccessArg::Remote => "remote",
            }
        );
        println!("Bind: {}", profile.bind);
        println!("Port: {}", profile.port);
        println!(
            "Token required: {}",
            if profile.token_required { "yes" } else { "no" }
        );
        if profile.token_required {
            println!("Token TTL (minutes): {}", profile.token_ttl_minutes);
        }
        return Ok(());
    }
    setup_web::run_setup_web(setup_web::SetupWebOptions {
        bundle_root: bundle_path,
        bind: profile.bind,
        port: profile.port,
        token_required: profile.token_required,
        token_ttl_minutes: profile.token_ttl_minutes,
        defaults,
    })
}

fn run_browser_setup_auto() -> anyhow::Result<()> {
    println!("Project folder: where runtime.toml, io.toml, src/, program.stbc live.");
    let default_bundle = default_bundle_path();
    let bundle_path = prompt::prompt_path("Project folder (runtime files)", &default_bundle)?;
    run_browser_setup_mode(
        SetupAccessArg::Local,
        Some(bundle_path),
        None,
        DEFAULT_SETUP_PORT,
        None,
        false,
    )
}
