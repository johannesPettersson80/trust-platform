fn run_cli_setup_interactive() -> anyhow::Result<()> {
    println!("Setup mode:");
    println!("  1) Guided setup (recommended)");
    println!("  2) Manual setup");
    let choice = prompt::prompt_string("Select option", "1")?;
    match choice.trim() {
        "1" => run_cli_guided_interactive(),
        "2" => run_cli_manual(),
        _ => anyhow::bail!("Invalid option. Expected 1 or 2. Tip: run trust-runtime setup again."),
    }
}

fn run_cli_guided_interactive() -> anyhow::Result<()> {
    println!("Project folder: where runtime.toml, io.toml, src/, program.stbc live.");
    let default_bundle = default_bundle_path();
    let bundle_path = prompt::prompt_path("Project folder (runtime files)", &default_bundle)?;
    let defaults = SetupDefaults::from_bundle(&bundle_path);
    let resource_name: String = prompt::prompt_string("PLC name", defaults.resource_name.as_str())?;
    let cycle_ms = prompt::prompt_u64("Cycle time (ms)", defaults.cycle_ms)?;
    let resource_name = SmolStr::new(resource_name);
    wizard::validate_runtime_generation_inputs(&resource_name, cycle_ms)?;
    wizard::create_bundle_auto(Some(bundle_path.clone()))?;
    println!(
        "{}",
        style::success(format!(
            "Project folder ready at: {}",
            bundle_path.display()
        ))
    );
    let write_system_io =
        prompt::prompt_yes_no("Write system-wide I/O config for this device?", true)?;
    if write_system_io {
        let overwrite = prompt::prompt_yes_no("Overwrite existing system-wide I/O config?", false)?;
        let options = trust_runtime::setup::SetupOptions {
            driver: Some(SmolStr::new(defaults.driver.clone())),
            backend: None,
            force: overwrite,
            path: None,
        };
        trust_runtime::setup::run_setup(options)?;
    }
    let use_system_io =
        prompt::prompt_yes_no("Use system-wide I/O config for this project?", true)?;
    let io_path = bundle_path.join("io.toml");
    if use_system_io {
        wizard::remove_io_toml(&io_path)?;
    } else {
        println!(
            "Choose gpio for Raspberry Pi, loopback/simulated for local runs, modbus-tcp for devices, mqtt for brokered exchange, or ethercat (mock for deterministic runs, NIC adapter for hardware) for EtherCAT module-chain validation."
        );
        let driver = prompt::prompt_string(
            "I/O driver (gpio, loopback, simulated, modbus-tcp, mqtt, ethercat)",
            &defaults.driver,
        )?;
        wizard::write_io_toml_with_driver(&io_path, driver.trim())?;
    }
    let runtime_path = bundle_path.join("runtime.toml");
    wizard::write_runtime_toml(&runtime_path, &resource_name, cycle_ms)?;
    print_setup_complete(&bundle_path);
    Ok(())
}

fn run_cli_guided_noninteractive(project: Option<PathBuf>, dry_run: bool) -> anyhow::Result<()> {
    let bundle_path = project.unwrap_or_else(default_bundle_path);
    let defaults = SetupDefaults::from_bundle(&bundle_path);
    if dry_run {
        println!("{}", style::accent("Setup dry run (CLI guided mode)"));
        println!("Project: {}", bundle_path.display());
        println!("PLC name: {}", defaults.resource_name);
        println!("Cycle (ms): {}", defaults.cycle_ms);
        println!("I/O driver: {}", defaults.driver);
        println!("Write system I/O: no");
        println!("Use project io.toml: yes");
        return Ok(());
    }
    wizard::create_bundle_auto(Some(bundle_path.clone()))?;
    let runtime_path = bundle_path.join("runtime.toml");
    wizard::write_runtime_toml(&runtime_path, &defaults.resource_name, defaults.cycle_ms)?;
    let io_path = bundle_path.join("io.toml");
    wizard::write_io_toml_with_driver(&io_path, defaults.driver.as_str())?;
    print_setup_complete(&bundle_path);
    Ok(())
}

fn run_cli_manual() -> anyhow::Result<()> {
    println!("Manual setup:");
    println!("1) Create a project folder with:");
    println!("   - runtime.toml ([bundle], [resource], [runtime.control], [runtime.log])");
    println!("   - program.stbc");
    println!("   - io.toml (optional if using system IO)");
    println!("2) Start runtime with:");
    println!("   trust-runtime --project <project-folder>");
    println!("3) System IO config (optional):");
    println!("   sudo trust-runtime setup");
    println!("Need help later? Run: trust-runtime setup");
    Ok(())
}
