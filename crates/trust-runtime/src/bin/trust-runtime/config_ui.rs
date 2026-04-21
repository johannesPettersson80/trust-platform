//! Standalone browser IDE server command (`trust-runtime ide serve`).

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use anyhow::Context;
use indexmap::IndexMap;
use smol_str::SmolStr;
use trust_runtime::config::{
    ControlMode, RuntimeCloudProfile, RuntimeConfig, WebAuthMode, WebConfig,
};
use trust_runtime::control::{ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry};
use trust_runtime::debug::{DebugSnapshot, DebugVariableHandles};
use trust_runtime::discovery::DiscoveryState;
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::linux_rt::LinuxRtRuntimeStatus;
use trust_runtime::metrics::RuntimeMetrics;
use trust_runtime::scheduler::{ResourceCommand, ResourceControl, StdClock};
use trust_runtime::settings::{
    BaseSettings, DiscoverySettings, MeshSettings, RuntimeSettings, SimulationSettings, WebSettings,
};
use trust_runtime::watchdog::{FaultPolicy, RetainMode, WatchdogPolicy};
use trust_runtime::web::{start_web_server_with_mode, WebServerMode};

pub fn run_config_ui_serve(project: Option<PathBuf>, listen: String) -> anyhow::Result<()> {
    eprintln!(
        "{}",
        crate::style::warning("Warning: `config-ui serve` is deprecated. Use `ide serve` instead.",)
    );
    run_ide_serve(project, listen)
}

pub fn run_ide_serve(project: Option<PathBuf>, listen: String) -> anyhow::Result<()> {
    let project_root = project
        .or_else(|| std::env::current_dir().ok())
        .ok_or_else(|| anyhow::anyhow!("failed to resolve project path"))?;
    if !project_root.exists() {
        anyhow::bail!("project path '{}' does not exist", project_root.display());
    }

    let runtime_id =
        detect_primary_runtime_id(&project_root).unwrap_or_else(|_| SmolStr::new("config-ui"));
    let control_state = build_config_mode_control_state(project_root.clone(), runtime_id, &listen)
        .context("build config UI control state")?;

    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: false,
    };
    let _server = start_web_server_with_mode(
        &config,
        control_state,
        Some(Arc::new(DiscoveryState::new())),
        None,
        Some(project_root.clone()),
        None,
        WebServerMode::StandaloneIde,
    )
    .map_err(|error| anyhow::anyhow!("start standalone IDE web server: {error}"))?;

    println!(
        "Standalone IDE running at http://{}/ide (mode=standalone-ide)",
        listen.trim()
    );
    println!("Project root: {}", project_root.display());
    println!("Press Ctrl+C to stop.");

    loop {
        thread::sleep(Duration::from_secs(3600));
    }
}

fn detect_primary_runtime_id(project_root: &Path) -> Result<SmolStr, RuntimeError> {
    let mut runtime_paths = Vec::new();
    let direct = project_root.join("runtime.toml");
    if direct.is_file() {
        runtime_paths.push(direct);
    }

    if let Ok(entries) = std::fs::read_dir(project_root) {
        for entry in entries.flatten() {
            let Ok(file_type) = entry.file_type() else {
                continue;
            };
            if !file_type.is_dir() {
                continue;
            }
            let candidate = entry.path().join("runtime.toml");
            if candidate.is_file() {
                runtime_paths.push(candidate);
            }
        }
    }

    runtime_paths.sort();
    let Some(path) = runtime_paths.first() else {
        return Err(RuntimeError::InvalidConfig(
            "no runtime.toml found for standalone IDE serve".into(),
        ));
    };
    let runtime = RuntimeConfig::load(path)?;
    Ok(runtime.resource_name)
}

fn build_config_mode_control_state(
    project_root: PathBuf,
    resource_name: SmolStr,
    listen: &str,
) -> anyhow::Result<Arc<ControlState>> {
    let mut harness = TestHarness::from_source("PROGRAM Main\nEND_PROGRAM\n")
        .map_err(|error| anyhow::anyhow!(error.to_string()))
        .context("create config-ui harness")?;
    let debug = harness.runtime_mut().enable_debug();
    harness.cycle();
    let snapshot = DebugSnapshot {
        storage: harness.runtime().storage().clone(),
        now: harness.runtime().current_time(),
    };

    let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
    thread::spawn(move || {
        while let Ok(command) = cmd_rx.recv() {
            match command {
                ResourceCommand::ReloadBytecode { respond_to, .. } => {
                    let _ = respond_to
                        .send(Err(RuntimeError::ControlError(SmolStr::new("unsupported"))));
                }
                ResourceCommand::MeshSnapshot { respond_to, .. } => {
                    let _ = respond_to.send(IndexMap::new());
                }
                ResourceCommand::Snapshot { respond_to } => {
                    let _ = respond_to.send(snapshot.clone());
                }
                _ => {}
            }
        }
    });

    let sources = load_source_registry(project_root.as_path());
    let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        Some(project_root.as_path()),
        &sources,
    )));

    let mut settings = RuntimeSettings::new(
        trust_runtime::value::Duration::from_millis(10),
        BaseSettings {
            log_level: SmolStr::new("info"),
            watchdog: WatchdogPolicy::default(),
            fault_policy: FaultPolicy::SafeHalt,
            retain_mode: RetainMode::None,
            retain_save_interval: None,
        },
        WebSettings {
            enabled: true,
            listen: SmolStr::new(listen),
            auth: SmolStr::new("local"),
            tls: false,
        },
        DiscoverySettings {
            enabled: false,
            service_name: resource_name.clone(),
            advertise: false,
            interfaces: Vec::new(),
            host_group: None,
        },
        MeshSettings {
            enabled: false,
            role: trust_runtime::config::MeshRole::Peer,
            listen: SmolStr::new("127.0.0.1:0"),
            connect: Vec::new(),
            tls: false,
            auth_token: None,
            publish: Vec::new(),
            subscribe: IndexMap::new(),
            zenohd_version: SmolStr::new("1.7.2"),
            plugin_versions: IndexMap::new(),
        },
        SimulationSettings {
            enabled: false,
            time_scale: 1,
            mode_label: SmolStr::new("production"),
            warning: SmolStr::new(""),
        },
    );
    settings.runtime_cloud.profile = RuntimeCloudProfile::Dev;
    settings.runtime_cloud.wan_allow_write = Vec::new();
    settings.runtime_cloud.link_preferences = Vec::new();

    Ok(Arc::new(ControlState {
        debug,
        resource,
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources,
        io_snapshot: Arc::new(Mutex::new(None)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(None)),
        control_requires_auth: false,
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(settings)),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            trust_runtime::linux_rt::LinuxRtConfig::default(),
        ))),
        project_root: Some(project_root),
        resource_name,
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(false)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_descriptor,
        historian: None,
        pairing: None,
    }))
}

fn load_source_registry(project_root: &Path) -> SourceRegistry {
    let src_root = project_root.join("src");
    let mut files = Vec::new();
    if let Ok(entries) = std::fs::read_dir(&src_root) {
        for (index, entry) in entries.flatten().enumerate() {
            let path = entry.path();
            if path.extension().and_then(|value| value.to_str()) != Some("st") {
                continue;
            }
            let Ok(text) = std::fs::read_to_string(&path) else {
                continue;
            };
            let relative = path
                .strip_prefix(project_root)
                .ok()
                .map(|p| p.to_path_buf())
                .unwrap_or(path.clone());
            files.push(SourceFile {
                id: index as u32 + 1,
                path: relative,
                text,
            });
        }
    }
    SourceRegistry::new(files)
}
