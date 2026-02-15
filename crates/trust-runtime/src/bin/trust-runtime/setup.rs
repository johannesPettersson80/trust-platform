//! Setup command handler.

use std::io::IsTerminal;
use std::net::IpAddr;
use std::path::{Path, PathBuf};

use smol_str::SmolStr;

use crate::cli::{SetupAccessArg, SetupModeArg};
use crate::prompt;
use crate::style;
use crate::wizard;

mod setup_web;

const DEFAULT_SETUP_PORT: u16 = 8080;
const DEFAULT_REMOTE_TOKEN_TTL_MINUTES: u64 = 15;
const MAX_REMOTE_TOKEN_TTL_MINUTES: u64 = 24 * 60;

#[derive(Debug, Clone)]
pub struct SetupCommandOptions {
    pub mode: Option<SetupModeArg>,
    pub access: SetupAccessArg,
    pub project: Option<PathBuf>,
    pub bind: Option<String>,
    pub port: u16,
    pub token_ttl_minutes: Option<u64>,
    pub dry_run: bool,
    pub driver: Option<String>,
    pub backend: Option<String>,
    pub path: Option<PathBuf>,
    pub force: bool,
}

#[derive(Debug, Clone)]
struct BrowserSetupProfile {
    bind: String,
    port: u16,
    token_required: bool,
    token_ttl_minutes: u64,
}

impl BrowserSetupProfile {
    fn build(
        access: SetupAccessArg,
        bind_override: Option<String>,
        port: u16,
        token_ttl_minutes: Option<u64>,
    ) -> anyhow::Result<Self> {
        let default_bind = match access {
            SetupAccessArg::Local => "127.0.0.1",
            SetupAccessArg::Remote => "0.0.0.0",
        };
        let bind = normalize_bind(bind_override.unwrap_or_else(|| default_bind.to_string()))?;
        match access {
            SetupAccessArg::Local => {
                if !is_loopback_bind(&bind) {
                    anyhow::bail!(
                        "local browser setup must use a loopback bind (127.0.0.1, ::1, localhost)"
                    );
                }
                if token_ttl_minutes.unwrap_or(0) > 0 {
                    anyhow::bail!(
                        "local browser setup must not set token TTL (tokens are remote-only)"
                    );
                }
                Ok(Self {
                    bind,
                    port,
                    token_required: false,
                    token_ttl_minutes: 0,
                })
            }
            SetupAccessArg::Remote => {
                if is_loopback_bind(&bind) {
                    anyhow::bail!(
                        "remote browser setup must not use a loopback bind; use 0.0.0.0 or a LAN address"
                    );
                }
                let ttl = token_ttl_minutes.unwrap_or(DEFAULT_REMOTE_TOKEN_TTL_MINUTES);
                if ttl == 0 {
                    anyhow::bail!("remote browser setup requires token_ttl_minutes > 0");
                }
                if ttl > MAX_REMOTE_TOKEN_TTL_MINUTES {
                    anyhow::bail!(
                        "token_ttl_minutes exceeds max allowed value ({MAX_REMOTE_TOKEN_TTL_MINUTES})"
                    );
                }
                Ok(Self {
                    bind,
                    port,
                    token_required: true,
                    token_ttl_minutes: ttl,
                })
            }
        }
    }
}

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
    access: SetupAccessArg,
    project: Option<PathBuf>,
    bind: Option<String>,
    port: u16,
    token_ttl_minutes: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    match mode {
        SetupModeArg::Cancel => {
            print_cancel_message();
            Ok(())
        }
        SetupModeArg::Browser => {
            run_browser_setup_mode(access, project, bind, port, token_ttl_minutes, dry_run)
        }
        SetupModeArg::Cli => run_cli_guided_noninteractive(project, dry_run),
    }
}

fn validate_system_setup_flag_mix(
    mode: Option<SetupModeArg>,
    access: SetupAccessArg,
    project: Option<&PathBuf>,
    bind: Option<&String>,
    port: u16,
    token_ttl_minutes: Option<u64>,
    dry_run: bool,
) -> anyhow::Result<()> {
    if mode.is_some()
        || project.is_some()
        || bind.is_some()
        || token_ttl_minutes.is_some()
        || !matches!(access, SetupAccessArg::Local)
        || port != DEFAULT_SETUP_PORT
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
        prompt::prompt_u64("Port", DEFAULT_SETUP_PORT.into())? as u16
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
    wizard::create_bundle_auto(Some(bundle_path.clone()))?;
    println!(
        "{}",
        style::success(format!(
            "Project folder ready at: {}",
            bundle_path.display()
        ))
    );
    let resource_name: String = prompt::prompt_string("PLC name", defaults.resource_name.as_str())?;
    let cycle_ms = prompt::prompt_u64("Cycle time (ms)", defaults.cycle_ms)?;
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
    wizard::write_runtime_toml(&runtime_path, &SmolStr::new(resource_name), cycle_ms)?;
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

fn print_cancel_message() {
    println!(
        "{}",
        style::warning("Setup cancelled. Resume any time with: trust-runtime setup")
    );
}

fn print_setup_complete(bundle_path: &Path) {
    println!("{}", style::success("✓ Setup complete!"));
    println!("Start the PLC with:");
    println!("  trust-runtime --project {}", bundle_path.display());
    println!("Open http://localhost:8080 to monitor.");
}

fn normalize_bind(bind: String) -> anyhow::Result<String> {
    let trimmed = bind.trim();
    if trimmed.is_empty() {
        anyhow::bail!("bind address must not be empty");
    }
    Ok(trimmed.to_string())
}

fn is_loopback_bind(bind: &str) -> bool {
    if bind.eq_ignore_ascii_case("localhost") || bind == "127.0.0.1" || bind == "::1" {
        return true;
    }
    bind.parse::<IpAddr>()
        .map(|addr| addr.is_loopback())
        .unwrap_or(false)
}

fn default_bundle_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("project")
}

#[derive(Debug, Clone)]
pub(crate) struct SetupDefaults {
    pub resource_name: SmolStr,
    pub cycle_ms: u64,
    pub driver: String,
}

impl SetupDefaults {
    pub fn from_bundle(root: &Path) -> Self {
        let resource_name = wizard::default_resource_name(root);
        let cycle_ms = 100;
        let driver = if trust_runtime::setup::is_raspberry_pi_hint() {
            "gpio"
        } else {
            "loopback"
        };
        Self {
            resource_name,
            cycle_ms,
            driver: driver.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_profile_local_enforces_loopback_and_no_token() {
        let profile =
            BrowserSetupProfile::build(SetupAccessArg::Local, None, DEFAULT_SETUP_PORT, None)
                .expect("local profile");
        assert_eq!(profile.bind, "127.0.0.1");
        assert!(!profile.token_required);
        assert_eq!(profile.token_ttl_minutes, 0);

        let err = BrowserSetupProfile::build(
            SetupAccessArg::Local,
            Some("0.0.0.0".to_string()),
            DEFAULT_SETUP_PORT,
            None,
        )
        .expect_err("local non-loopback must fail");
        assert!(err.to_string().contains("loopback"));
    }

    #[test]
    fn browser_profile_remote_requires_non_loopback_and_token_ttl() {
        let profile =
            BrowserSetupProfile::build(SetupAccessArg::Remote, None, DEFAULT_SETUP_PORT, None)
                .expect("remote profile");
        assert_eq!(profile.bind, "0.0.0.0");
        assert!(profile.token_required);
        assert_eq!(profile.token_ttl_minutes, DEFAULT_REMOTE_TOKEN_TTL_MINUTES);

        let loopback_err = BrowserSetupProfile::build(
            SetupAccessArg::Remote,
            Some("127.0.0.1".to_string()),
            DEFAULT_SETUP_PORT,
            Some(15),
        )
        .expect_err("remote loopback must fail");
        assert!(loopback_err
            .to_string()
            .contains("must not use a loopback bind"));

        let ttl_err =
            BrowserSetupProfile::build(SetupAccessArg::Remote, None, DEFAULT_SETUP_PORT, Some(0))
                .expect_err("remote ttl zero must fail");
        assert!(ttl_err.to_string().contains("token_ttl_minutes > 0"));
    }
}
