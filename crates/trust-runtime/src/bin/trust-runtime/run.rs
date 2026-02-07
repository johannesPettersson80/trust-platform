//! Runtime launcher helpers.

use std::collections::VecDeque;
use std::io::IsTerminal;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::json;
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::config::RuntimeBundle;
use trust_runtime::control::{
    ControlEndpoint, ControlServer, ControlState, SourceFile, SourceRegistry,
};
use trust_runtime::discovery::{start_discovery, DiscoveryState};
use trust_runtime::harness::CompileSession;
use trust_runtime::io::IoDriverRegistry;
use trust_runtime::mesh::start_mesh;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::retain::FileRetainStore;
use trust_runtime::scheduler::{ResourceRunner, StartGate, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::value::Duration;
use trust_runtime::web::pairing::PairingStore;
use trust_runtime::web::start_web_server;
use trust_runtime::{RestartMode, Runtime};

use crate::setup;
use crate::style;
use crate::wizard;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleMode {
    Auto,
    Enabled,
    Disabled,
}

pub fn run_default(verbose: bool) -> anyhow::Result<()> {
    match detect_bundle_path(None) {
        Ok(path) => run_play(
            Some(path),
            "cold".to_string(),
            verbose,
            ConsoleMode::Auto,
            false,
            false,
            1,
        ),
        Err(_) => {
            if std::io::stdin().is_terminal() {
                setup::run_setup_default()
            } else {
                run_play(
                    None,
                    "cold".to_string(),
                    verbose,
                    ConsoleMode::Disabled,
                    false,
                    false,
                    1,
                )
            }
        }
    }
}

pub fn run_play(
    project: Option<PathBuf>,
    restart: String,
    verbose: bool,
    console: ConsoleMode,
    beginner: bool,
    simulation: bool,
    time_scale: u32,
) -> anyhow::Result<()> {
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
        restart,
        verbose,
        true,
        console,
        beginner,
        simulation,
        time_scale,
    )
}

pub fn run_validate(bundle: PathBuf, ci: bool) -> anyhow::Result<()> {
    let bundle = RuntimeBundle::load(&bundle)?;
    let control_endpoint = ControlEndpoint::parse(bundle.runtime.control_endpoint.as_str())?;
    if matches!(control_endpoint, ControlEndpoint::Tcp(_))
        && bundle.runtime.control_auth_token.is_none()
    {
        anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
    }
    let registry = IoDriverRegistry::default_registry();
    registry
        .validate(&bundle.io.driver, &bundle.io.params)
        .map_err(anyhow::Error::from)?;
    let mut runtime = Runtime::new();
    runtime.apply_bytecode_bytes(&bundle.bytecode, Some(&bundle.runtime.resource_name))?;
    if ci {
        let payload = json!({
            "version": 1,
            "command": "validate",
            "status": "ok",
            "project": bundle.root.display().to_string(),
            "resource": bundle.runtime.resource_name.to_string(),
            "control_endpoint": bundle.runtime.control_endpoint.to_string(),
            "io_driver": bundle.io.driver.to_string(),
        });
        println!("{}", serde_json::to_string_pretty(&payload)?);
        return Ok(());
    }
    println!("{}", style::success("Project ok"));
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn run_runtime(
    project: Option<PathBuf>,
    config: Option<PathBuf>,
    runtime_root: Option<PathBuf>,
    restart: String,
    verbose: bool,
    show_banner: bool,
    console: ConsoleMode,
    beginner: bool,
    simulation: bool,
    time_scale: u32,
) -> anyhow::Result<()> {
    let restart_mode = match restart.to_ascii_lowercase().as_str() {
        "cold" => RestartMode::Cold,
        "warm" => RestartMode::Warm,
        _ => anyhow::bail!(
            "Invalid restart mode: {restart}. Expected: cold or warm. Tip: run trust-runtime play --help"
        ),
    };

    let (bundle, mut runtime, sources) = if let Some(project_path) = project {
        let bundle = RuntimeBundle::load(&project_path)?;
        let sources_path = bundle.root.join("sources");
        if sources_path.is_dir() {
            let sources = load_sources(&sources_path)?;
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
            let runtime = session.build_runtime()?;
            (Some(bundle), runtime, sources)
        } else {
            let runtime = Runtime::new();
            let sources = SourceRegistry::default();
            (Some(bundle), runtime, sources)
        }
    } else {
        let config_path = config.ok_or_else(|| anyhow::anyhow!("--config required"))?;
        let runtime_root = runtime_root.unwrap_or_else(|| {
            config_path
                .parent()
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from("."))
        });
        let sources = load_sources(&runtime_root)?;
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
        let runtime = session.build_runtime()?;
        (None, runtime, sources)
    };

    if time_scale == 0 {
        anyhow::bail!("--time-scale must be >= 1");
    }
    let mut simulation_config = bundle
        .as_ref()
        .and_then(|bundle| bundle.simulation.clone())
        .unwrap_or_default();
    if simulation || time_scale > 1 {
        simulation_config.enabled = true;
    }
    if time_scale > 1 {
        simulation_config.time_scale = time_scale;
    }
    if simulation_config.time_scale == 0 {
        anyhow::bail!("simulation.time_scale must be >= 1");
    }
    let simulation_enabled = simulation_config.enabled;
    let simulation_time_scale = simulation_config.time_scale.max(1);
    let simulation_warning =
        simulation_warning_message(simulation_enabled, simulation_time_scale).unwrap_or_default();
    let simulation_controller = simulation_enabled
        .then(|| trust_runtime::simulation::SimulationController::new(simulation_config));

    let debug = runtime.enable_debug();
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::new()));
    runtime.set_metrics_sink(metrics.clone());
    let io_health = Arc::new(Mutex::new(Vec::new()));
    runtime.set_io_health_sink(Some(io_health.clone()));
    let io_snapshot = Arc::new(Mutex::new(None));
    let (io_tx, io_rx) = std::sync::mpsc::channel();
    debug.set_io_sender(io_tx);
    {
        let io_snapshot = io_snapshot.clone();
        std::thread::spawn(move || {
            for snapshot in io_rx {
                if let Ok(mut guard) = io_snapshot.lock() {
                    *guard = Some(snapshot);
                }
            }
        });
    }
    if let Some(bundle) = &bundle {
        if bundle.runtime.bundle_version != 1 {
            anyhow::bail!(
                "unsupported bundle version {}",
                bundle.runtime.bundle_version
            );
        }
        runtime.set_watchdog_policy(bundle.runtime.watchdog);
        runtime.set_fault_policy(bundle.runtime.fault_policy);
        runtime.set_io_safe_state(bundle.io.safe_state.clone());
        let registry = IoDriverRegistry::default_registry();
        if let Some(spec) = registry
            .build(&bundle.io.driver, &bundle.io.params)
            .map_err(anyhow::Error::from)?
        {
            runtime.add_io_driver(spec.name, spec.driver);
        }
        match bundle.runtime.retain_mode {
            trust_runtime::watchdog::RetainMode::File => {
                let store = bundle.runtime.retain_path.as_ref().map(|path| {
                    let path = if path.is_relative() {
                        bundle.root.join(path)
                    } else {
                        path.clone()
                    };
                    Box::new(FileRetainStore::new(path)) as _
                });
                runtime.set_retain_store(store, Some(bundle.runtime.retain_save_interval));
            }
            trust_runtime::watchdog::RetainMode::None => {
                runtime.set_retain_store(None, None);
            }
        }
        if let Err(err) =
            runtime.apply_bytecode_bytes(&bundle.bytecode, Some(&bundle.runtime.resource_name))
        {
            anyhow::bail!(
                "failed to apply bytecode metadata: {err} (project folder may require sources)"
            );
        }
    }

    runtime.restart(restart_mode)?;
    runtime.load_retain_store()?;

    let logger = RuntimeLogger::new(match &bundle {
        Some(bundle) => LogLevel::parse(bundle.runtime.log_level.as_str()),
        None => LogLevel::Info,
    });

    let metadata = Arc::new(Mutex::new(runtime.metadata_snapshot()));
    let events = Arc::new(Mutex::new(VecDeque::new()));
    {
        let events = events.clone();
        let (event_tx, event_rx) = std::sync::mpsc::channel();
        debug.set_runtime_sender(event_tx);
        let event_logger = logger.clone();
        std::thread::spawn(move || {
            for event in event_rx {
                log_runtime_event(&event_logger, &event);
                if let Ok(mut guard) = events.lock() {
                    guard.push_back(event);
                    while guard.len() > 200 {
                        guard.pop_front();
                    }
                }
            }
        });
    }
    let pending_restart = Arc::new(Mutex::new(None));
    let start_gate = Arc::new(StartGate::new());

    let control_endpoint = if let Some(bundle) = &bundle {
        ControlEndpoint::parse(bundle.runtime.control_endpoint.as_str())?
    } else {
        ControlEndpoint::parse("tcp://127.0.0.1:9000")?
    };
    if matches!(control_endpoint, ControlEndpoint::Tcp(_)) {
        let token = bundle
            .as_ref()
            .and_then(|bundle| bundle.runtime.control_auth_token.as_ref());
        if token.is_none() {
            anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
        }
    }

    let default_watchdog = runtime.watchdog_policy();
    let default_fault = runtime.fault_policy();
    let cycle_interval = bundle
        .as_ref()
        .map(|bundle| bundle.runtime.cycle_interval)
        .unwrap_or_else(|| Duration::from_millis(10));
    let mut runner = ResourceRunner::new(runtime, StdClock::new(), cycle_interval)
        .with_restart_signal(pending_restart.clone())
        .with_start_gate(start_gate.clone())
        .with_time_scale(simulation_time_scale);
    if let Some(simulation) = simulation_controller {
        runner = runner.with_simulation(simulation);
    }
    let mut handle = runner.spawn("trust-runtime")?;
    let control = handle.control();

    let settings = if let Some(bundle) = &bundle {
        RuntimeSettings::new(
            BaseSettings {
                log_level: bundle.runtime.log_level.clone(),
                watchdog: bundle.runtime.watchdog,
                fault_policy: bundle.runtime.fault_policy,
                retain_mode: bundle.runtime.retain_mode,
                retain_save_interval: Some(bundle.runtime.retain_save_interval),
            },
            WebSettings {
                enabled: bundle.runtime.web.enabled,
                listen: bundle.runtime.web.listen.clone(),
                auth: SmolStr::new(match bundle.runtime.web.auth {
                    trust_runtime::config::WebAuthMode::Local => "local",
                    trust_runtime::config::WebAuthMode::Token => "token",
                }),
            },
            DiscoverySettings {
                enabled: bundle.runtime.discovery.enabled,
                service_name: bundle.runtime.discovery.service_name.clone(),
                advertise: bundle.runtime.discovery.advertise,
                interfaces: bundle.runtime.discovery.interfaces.clone(),
            },
            MeshSettings {
                enabled: bundle.runtime.mesh.enabled,
                listen: bundle.runtime.mesh.listen.clone(),
                auth_token: bundle.runtime.mesh.auth_token.clone(),
                publish: bundle.runtime.mesh.publish.clone(),
                subscribe: bundle.runtime.mesh.subscribe.clone(),
            },
            SimulationSettings {
                enabled: simulation_enabled,
                time_scale: simulation_time_scale,
                mode_label: SmolStr::new(if simulation_enabled {
                    "simulation"
                } else {
                    "production"
                }),
                warning: SmolStr::new(simulation_warning.clone()),
            },
        )
    } else {
        RuntimeSettings::new(
            BaseSettings {
                log_level: SmolStr::new("info"),
                watchdog: default_watchdog,
                fault_policy: default_fault,
                retain_mode: trust_runtime::watchdog::RetainMode::None,
                retain_save_interval: None,
            },
            WebSettings {
                enabled: false,
                listen: SmolStr::new("0.0.0.0:8080"),
                auth: SmolStr::new("local"),
            },
            DiscoverySettings {
                enabled: false,
                service_name: SmolStr::new("truST"),
                advertise: false,
                interfaces: Vec::new(),
            },
            MeshSettings {
                enabled: false,
                listen: SmolStr::new("0.0.0.0:5200"),
                auth_token: None,
                publish: Vec::new(),
                subscribe: indexmap::IndexMap::new(),
            },
            SimulationSettings {
                enabled: simulation_enabled,
                time_scale: simulation_time_scale,
                mode_label: SmolStr::new(if simulation_enabled {
                    "simulation"
                } else {
                    "production"
                }),
                warning: SmolStr::new(simulation_warning.clone()),
            },
        )
    };
    let auth_token_value = bundle
        .as_ref()
        .and_then(|bundle| bundle.runtime.control_auth_token.as_ref())
        .map(|token| token.to_string());
    let auth_token = Arc::new(Mutex::new(
        bundle
            .as_ref()
            .and_then(|bundle| bundle.runtime.control_auth_token.clone()),
    ));
    let pairing = bundle
        .as_ref()
        .map(|bundle| Arc::new(PairingStore::load(bundle.root.join("pairings.json"))));
    let (audit_tx, audit_rx) = std::sync::mpsc::channel();
    let audit_logger = logger.clone();
    std::thread::spawn(move || {
        for event in audit_rx {
            log_control_audit(&audit_logger, event);
        }
    });

    let state = Arc::new(ControlState {
        debug: debug.clone(),
        resource: control.clone(),
        metadata: metadata.clone(),
        sources,
        io_snapshot: io_snapshot.clone(),
        pending_restart,
        auth_token: auth_token.clone(),
        control_requires_auth: matches!(control_endpoint, ControlEndpoint::Tcp(_)),
        control_mode: Arc::new(Mutex::new(
            bundle
                .as_ref()
                .map(|bundle| bundle.runtime.control_mode)
                .unwrap_or(trust_runtime::config::ControlMode::Debug),
        )),
        audit_tx: Some(audit_tx),
        metrics: metrics.clone(),
        events: events.clone(),
        settings: Arc::new(Mutex::new(settings)),
        project_root: bundle.as_ref().map(|bundle| bundle.root.clone()),
        resource_name: bundle
            .as_ref()
            .map(|bundle| bundle.runtime.resource_name.clone())
            .unwrap_or_else(|| smol_str::SmolStr::new("RESOURCE")),
        io_health: io_health.clone(),
        debug_enabled: std::sync::Arc::new(std::sync::atomic::AtomicBool::new(
            bundle
                .as_ref()
                .map(|bundle| bundle.runtime.control_debug_enabled)
                .unwrap_or(true),
        )),
        debug_variables: Arc::new(Mutex::new(trust_runtime::debug::DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        pairing: pairing.clone(),
    });

    let _server = ControlServer::start(control_endpoint.clone(), state.clone())?;
    let _discovery_handle = if let Some(bundle) = &bundle {
        if bundle.runtime.discovery.enabled {
            let web_listen = bundle.runtime.web.listen.as_str();
            let mesh_listen = bundle.runtime.mesh.listen.as_str();
            let handle = start_discovery(
                &bundle.runtime.discovery,
                &bundle.runtime.resource_name,
                &control_endpoint,
                Some(web_listen),
                Some(mesh_listen),
            )?;
            Some(handle)
        } else {
            None
        }
    } else {
        None
    };
    let discovery_state = _discovery_handle
        .as_ref()
        .map(|handle| handle.state())
        .unwrap_or_else(|| Arc::new(DiscoveryState::new()));
    let _web = if let Some(bundle) = &bundle {
        if bundle.runtime.web.enabled {
            Some(start_web_server(
                &bundle.runtime.web,
                state.clone(),
                Some(discovery_state.clone()),
                pairing.clone(),
                Some(bundle.root.clone()),
            )?)
        } else {
            None
        }
    } else {
        None
    };
    let _mesh = if let Some(bundle) = &bundle {
        start_mesh(
            &bundle.runtime.mesh,
            bundle.runtime.resource_name.clone(),
            control.clone(),
            Some(discovery_state.clone()),
        )?
    } else {
        None
    };
    start_gate.open();

    if show_banner {
        let web_url = bundle
            .as_ref()
            .filter(|bundle| bundle.runtime.web.enabled)
            .map(|bundle| format_web_url(bundle.runtime.web.listen.as_str()));
        print_trust_banner(
            bundle.as_ref(),
            web_url.as_deref(),
            simulation_enabled,
            simulation_time_scale,
        );
    }

    let wants_console = match console {
        ConsoleMode::Auto => std::io::stdin().is_terminal() && std::io::stdout().is_terminal(),
        ConsoleMode::Enabled => true,
        ConsoleMode::Disabled => false,
    };
    if wants_console {
        if !std::io::stdin().is_terminal() || !std::io::stdout().is_terminal() {
            anyhow::bail!("interactive console requires a TTY (use --no-console)");
        }
        let bundle_root = bundle.as_ref().map(|bundle| bundle.root.clone());
        if bundle_root.is_none() {
            anyhow::bail!("interactive console requires a project bundle");
        }
        let endpoint = format_endpoint(&control_endpoint);
        trust_runtime::ui::run_ui(
            bundle_root,
            Some(endpoint),
            auth_token_value.clone(),
            250,
            false,
            beginner,
        )?;
        println!("Console closed. Runtime still running. Press Ctrl+C to stop.");
    }
    if let Some(bundle) = &bundle {
        if verbose {
            print_startup_summary(
                bundle,
                restart_mode,
                &control_endpoint,
                simulation_enabled,
                simulation_time_scale,
            );
        }
        logger.log(
            LogLevel::Debug,
            "runtime_start",
            json!({
                "project": bundle.root.display().to_string(),
                "project_version": bundle.runtime.bundle_version,
                "resource": bundle.runtime.resource_name.to_string(),
                "restart": format!("{restart_mode:?}"),
                "cycle_interval_ms": bundle.runtime.cycle_interval.as_millis(),
                "io_driver": bundle.io.driver.to_string(),
                "retain_mode": format_retain_mode(bundle.runtime.retain_mode),
                "retain_path": bundle.runtime.retain_path.as_ref().map(|p| p.display().to_string()),
                "retain_save_ms": bundle.runtime.retain_save_interval.as_millis(),
                "watchdog_enabled": bundle.runtime.watchdog.enabled,
                "watchdog_timeout_ms": bundle.runtime.watchdog.timeout.as_millis(),
                "watchdog_action": format!("{:?}", bundle.runtime.watchdog.action),
                "fault_policy": format!("{:?}", bundle.runtime.fault_policy),
                "control_endpoint": format_endpoint(&control_endpoint),
                "control_auth_token_set": bundle.runtime.control_auth_token.is_some(),
                "control_auth_token_length": bundle.runtime.control_auth_token.as_ref().map(|t| t.len()),
                "control_debug_enabled": bundle.runtime.control_debug_enabled,
                "control_mode": format!("{:?}", bundle.runtime.control_mode),
                "web_enabled": bundle.runtime.web.enabled,
                "web_listen": bundle.runtime.web.listen.to_string(),
                "discovery_enabled": bundle.runtime.discovery.enabled,
                "mesh_enabled": bundle.runtime.mesh.enabled,
                "simulation_mode": if simulation_enabled { "simulation" } else { "production" },
                "simulation_time_scale": simulation_time_scale,
            }),
        );
    }

    handle
        .join()
        .map_err(|_| anyhow::anyhow!("runtime thread panicked"))?;
    logger.log(
        LogLevel::Debug,
        "runtime_exit",
        json!({ "status": "stopped" }),
    );
    Ok(())
}

fn print_trust_banner(
    bundle: Option<&RuntimeBundle>,
    web_url: Option<&str>,
    simulation_enabled: bool,
    simulation_time_scale: u32,
) {
    crate::style::print_logo();
    println!("Your PLC is running.");
    if let Some(warning) = simulation_warning_message(simulation_enabled, simulation_time_scale) {
        println!("{}", style::warning(warning));
    }
    if let Some(bundle) = bundle {
        println!("PLC name: {}", bundle.runtime.resource_name);
        println!("Project: {}", bundle.root.display());
        println!("I/O driver: {}", bundle.io.driver);
        println!(
            "Control mode: {:?} (debug {})",
            bundle.runtime.control_mode,
            if bundle.runtime.control_debug_enabled {
                "on"
            } else {
                "off"
            }
        );
    }
    if let Some(web_url) = web_url {
        println!("Open: {web_url}");
    } else {
        println!("Web UI: disabled");
    }
    println!("Press Ctrl+C to stop.");
}

fn format_endpoint(endpoint: &ControlEndpoint) -> String {
    match endpoint {
        ControlEndpoint::Tcp(addr) => format!("tcp://{addr}"),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => format!("unix://{}", path.display()),
    }
}

fn load_sources(root: &Path) -> anyhow::Result<SourceRegistry> {
    let mut files = Vec::new();
    let patterns = ["**/*.st", "**/*.ST", "**/*.pou", "**/*.POU"];
    for pattern in patterns {
        for entry in glob::glob(&format!("{}/{}", root.display(), pattern))? {
            let path = entry?;
            if files.iter().any(|file: &SourceFile| file.path == path) {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            let id = files.len() as u32;
            files.push(SourceFile { id, path, text });
        }
    }
    Ok(SourceRegistry::new(files))
}

fn print_startup_summary(
    bundle: &RuntimeBundle,
    restart: RestartMode,
    endpoint: &ControlEndpoint,
    simulation_enabled: bool,
    simulation_time_scale: u32,
) {
    println!("project folder: {}", bundle.root.display());
    println!("PLC name: {}", bundle.runtime.resource_name);
    println!("restart: {restart:?}");
    println!(
        "mode: {} (time scale x{})",
        if simulation_enabled {
            "simulation"
        } else {
            "production"
        },
        simulation_time_scale
    );
    println!(
        "cycle interval: {} ms",
        bundle.runtime.cycle_interval.as_millis()
    );
    println!("io driver: {}", bundle.io.driver);
    println!("control mode: {:?}", bundle.runtime.control_mode);
    println!(
        "debug: {}",
        if bundle.runtime.control_debug_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(token) = bundle.runtime.control_auth_token.as_ref() {
        println!("control auth: set (len={})", token.len());
    } else {
        println!("control auth: none");
    }
    println!(
        "retain: {} {}",
        format_retain_mode(bundle.runtime.retain_mode),
        bundle
            .runtime
            .retain_path
            .as_ref()
            .map(|path| format!("({})", path.display()))
            .unwrap_or_default()
    );
    println!(
        "retain save: {} ms",
        bundle.runtime.retain_save_interval.as_millis()
    );
    println!(
        "watchdog: enabled={} timeout={} ms action={:?}",
        bundle.runtime.watchdog.enabled,
        bundle.runtime.watchdog.timeout.as_millis(),
        bundle.runtime.watchdog.action
    );
    println!("fault policy: {:?}", bundle.runtime.fault_policy);
    println!("control endpoint: {}", format_endpoint(endpoint));
    println!(
        "web ui: {} ({})",
        if bundle.runtime.web.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.web.listen
    );
    println!(
        "discovery: {} ({})",
        if bundle.runtime.discovery.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.discovery.service_name
    );
    println!(
        "mesh: {} ({})",
        if bundle.runtime.mesh.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.mesh.listen
    );
}

fn format_retain_mode(mode: trust_runtime::watchdog::RetainMode) -> &'static str {
    match mode {
        trust_runtime::watchdog::RetainMode::None => "none",
        trust_runtime::watchdog::RetainMode::File => "file",
    }
}

fn format_web_url(listen: &str) -> String {
    let host = listen.split(':').next().unwrap_or("localhost");
    let port = listen.rsplit(':').next().unwrap_or("8080");
    let host = if host == "0.0.0.0" { "localhost" } else { host };
    format!("http://{host}:{port}")
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

#[cfg(test)]
mod tests {
    use super::simulation_warning_message;

    #[test]
    fn simulation_warning_includes_mode_and_safety_note() {
        let message = simulation_warning_message(true, 8).expect("message");
        assert!(message.contains("Simulation mode active"));
        assert!(message.contains("Not for live hardware"));
        assert!(message.contains("x8"));
    }

    #[test]
    fn simulation_warning_omitted_in_production_mode() {
        assert!(simulation_warning_message(false, 1).is_none());
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error,
    Warn,
    Info,
    Debug,
    Trace,
}

impl LogLevel {
    fn parse(text: &str) -> Self {
        match text.trim().to_ascii_lowercase().as_str() {
            "error" => Self::Error,
            "warn" | "warning" => Self::Warn,
            "debug" => Self::Debug,
            "trace" => Self::Trace,
            _ => Self::Info,
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::Error => "error",
            Self::Warn => "warn",
            Self::Info => "info",
            Self::Debug => "debug",
            Self::Trace => "trace",
        }
    }
}

#[derive(Debug, Clone)]
struct RuntimeLogger {
    level: LogLevel,
}

impl RuntimeLogger {
    fn new(level: LogLevel) -> Self {
        Self { level }
    }

    fn enabled(&self, level: LogLevel) -> bool {
        level <= self.level
    }

    fn log(&self, level: LogLevel, event: &str, data: serde_json::Value) {
        if !self.enabled(level) {
            return;
        }
        let timestamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let payload = json!({
            "ts": timestamp,
            "level": level.as_str(),
            "event": event,
            "data": data,
        });
        println!("{payload}");
    }
}

fn log_runtime_event(logger: &RuntimeLogger, event: &trust_runtime::debug::RuntimeEvent) {
    match event {
        trust_runtime::debug::RuntimeEvent::TaskOverrun { name, missed, time } => {
            logger.log(
                LogLevel::Warn,
                "runtime_overrun",
                json!({
                    "event_id": "TRUST-RT-OVERRUN-001",
                    "task": name.as_str(),
                    "missed": missed,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        trust_runtime::debug::RuntimeEvent::Fault { error, time } => {
            logger.log(
                LogLevel::Error,
                "runtime_fault",
                json!({
                    "event_id": "TRUST-RT-FAULT-001",
                    "error": error,
                    "time_ms": time.as_millis(),
                }),
            );
        }
        _ => {}
    }
}

fn log_control_audit(logger: &RuntimeLogger, event: trust_runtime::control::ControlAuditEvent) {
    logger.log(
        LogLevel::Debug,
        "control_audit",
        json!({
            "request_id": event.request_id,
            "request_type": event.request_type.as_str(),
            "ok": event.ok,
            "error": event.error.as_ref().map(|err| err.as_str()),
            "auth_present": event.auth_present,
            "client": event.client.as_ref().map(|client| client.as_str()),
            "timestamp_ms": event.timestamp_ms,
        }),
    );
}
