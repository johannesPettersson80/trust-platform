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
use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::config::{
    ControlMode, RuntimeCloudProfile, RuntimeConfig, WebAuthMode, WebConfig,
};
use trust_runtime::control::{
    AdsDoctorJobStore, ControlState, HmiRuntimeDescriptor, SourceFile, SourceRegistry,
};
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
    if !project_root.is_dir() {
        anyhow::bail!(
            "project path '{}' is not a directory",
            project_root.display()
        );
    }

    let runtime_id = if primary_runtime_config_path(&project_root).is_some() {
        detect_primary_runtime_id(&project_root)?
    } else {
        SmolStr::new("config-ui")
    };
    let control_state = build_config_mode_control_state(project_root.clone(), runtime_id, &listen)
        .context("build config UI control state")?;

    let config = WebConfig {
        enabled: true,
        listen: SmolStr::new(listen.clone()),
        auth: WebAuthMode::Local,
        tls: false,
    };
    let discovery = control_state.discovery.clone();
    let _server = start_web_server_with_mode(
        &config,
        control_state,
        Some(discovery),
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
    let direct = project_root.join("runtime.toml");
    if direct.is_file() {
        let runtime = RuntimeConfig::load(direct)?;
        return Ok(runtime.resource_name);
    }

    let Some(path) = primary_runtime_config_path(project_root) else {
        return Err(RuntimeError::InvalidConfig(
            "no runtime.toml found for standalone IDE serve".into(),
        ));
    };
    let runtime = RuntimeConfig::load(path)?;
    Ok(runtime.resource_name)
}

fn primary_runtime_config_path(project_root: &Path) -> Option<PathBuf> {
    let direct = project_root.join("runtime.toml");
    if direct.is_file() {
        return Some(direct);
    }

    let mut runtime_paths = Vec::new();
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
    runtime_paths.into_iter().next()
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
                ResourceCommand::AdsStatus { respond_to } => {
                    let _ = respond_to.send(trust_runtime::ads::diagnostics::AdsStatusReport {
                        schema_version:
                            trust_runtime::ads::diagnostics::ADS_DIAGNOSTICS_SCHEMA_VERSION,
                        role: trust_runtime::ads::diagnostics::DoctorRole::Client,
                        overall: trust_runtime::ads::diagnostics::AdsStatusOverall::Disabled,
                        runtime_identity_hash: None,
                        deployed_ads_config_hash: None,
                        connections: Vec::new(),
                        summary: "ADS is not configured.".to_string(),
                    });
                }
                ResourceCommand::ActiveAdsDevice { respond_to, .. } => {
                    let _ = respond_to.send(None);
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

    let discovery = Arc::new(DiscoveryState::new());
    Ok(Arc::new(ControlState {
        debug,
        resource,
        metadata: Arc::new(Mutex::new(harness.runtime().metadata_snapshot())),
        sources,
        io_snapshot: Arc::new(Mutex::new(None)),
        io_snapshot_seen_ms: Arc::new(std::sync::atomic::AtomicU64::new(0)),
        pending_restart: Arc::new(Mutex::new(None)),
        auth_token: Arc::new(Mutex::new(None)),
        control_requires_auth: false,
        control_mode: Arc::new(Mutex::new(ControlMode::Debug)),
        audit_tx: None,
        metrics: Arc::new(Mutex::new(RuntimeMetrics::default())),
        events: Arc::new(Mutex::new(VecDeque::new())),
        settings: Arc::new(Mutex::new(settings)),
        discovery,
        mesh_topology: Arc::new(Mutex::new(None)),
        realtime_status: Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
            trust_runtime::linux_rt::LinuxRtConfig::default(),
        ))),
        web_listener_bound: Arc::new(AtomicBool::new(false)),
        opcua_server_bound: Arc::new(AtomicBool::new(false)),
        project_root: Some(project_root),
        resource_name,
        io_health: Arc::new(Mutex::new(Vec::new())),
        debug_enabled: Arc::new(AtomicBool::new(false)),
        debug_variables: Arc::new(Mutex::new(DebugVariableHandles::new())),
        hmi_live: Arc::new(Mutex::new(trust_runtime::hmi::HmiLiveState::default())),
        hmi_persistence: None,
        hmi_descriptor,
        historian: None,
        openot_persistence_status: None,
        pairing: None,
        ads_doctor_jobs: Arc::new(Mutex::new(AdsDoctorJobStore::default())),
        ads_client_config: Arc::new(Mutex::new(None)),
        opcua_client_config: Arc::new(Mutex::new(None)),
        ads_server_config: Arc::new(Mutex::new(None)),
        #[cfg(feature = "ads-server")]
        ads_server_runtime: Arc::new(Mutex::new(None)),
    }))
}

fn load_source_registry(project_root: &Path) -> SourceRegistry {
    let source_project = primary_runtime_config_path(project_root)
        .and_then(|path| path.parent().map(Path::to_path_buf))
        .unwrap_or_else(|| project_root.to_path_buf());
    let canonical_workspace = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let files = collect_project_source_files(source_project.as_path(), None)
        .unwrap_or_default()
        .into_iter()
        .enumerate()
        .filter_map(|(index, source)| {
            let path = PathBuf::from(source.path?);
            let relative = path
                .strip_prefix(&canonical_workspace)
                .map(Path::to_path_buf)
                .unwrap_or(path);
            Some(SourceFile {
                id: index as u32 + 1,
                path: relative,
                text: source.text,
            })
        })
        .collect();
    SourceRegistry::new(files)
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;
    use trust_runtime::bundle_template::render_runtime_toml;

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "trust-runtime-config-ui-{name}-{}-{stamp}",
            std::process::id()
        ));
        std::fs::create_dir_all(&path).expect("create config UI test directory");
        path
    }

    fn write_runtime(project: &Path, resource_name: &str) {
        std::fs::create_dir_all(project).expect("create runtime project");
        std::fs::write(
            project.join("runtime.toml"),
            render_runtime_toml(&SmolStr::new(resource_name), 10),
        )
        .expect("write runtime.toml");
    }

    #[test]
    fn primary_runtime_detection_prefers_the_explicit_project_root() {
        let workspace = temp_dir("primary-root");
        write_runtime(&workspace, "root-runtime");
        write_runtime(&workspace.join("aaa-child"), "child-runtime");

        assert_eq!(
            detect_primary_runtime_id(&workspace).expect("detect primary runtime"),
            SmolStr::new("root-runtime")
        );

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn source_registry_uses_the_selected_workspace_runtime_and_build_source_order() {
        let workspace = temp_dir("workspace-sources");
        let runtime = workspace.join("runtime-a");
        write_runtime(&runtime, "runtime-a");
        std::fs::create_dir_all(runtime.join("src/nested")).expect("create nested sources");
        std::fs::write(runtime.join("src/zeta.st"), "PROGRAM Zeta\nEND_PROGRAM\n")
            .expect("write zeta source");
        std::fs::write(
            runtime.join("src/nested/alpha.ST"),
            "PROGRAM Alpha\nEND_PROGRAM\n",
        )
        .expect("write alpha source");
        std::fs::write(
            runtime.join("src/function.pou"),
            "FUNCTION Answer : INT\nAnswer := 42;\nEND_FUNCTION\n",
        )
        .expect("write pou source");

        let registry = load_source_registry(&workspace);
        let paths = registry
            .files()
            .iter()
            .map(|file| file.path.to_string_lossy().replace('\\', "/"))
            .collect::<Vec<_>>();
        assert_eq!(
            paths,
            vec![
                "runtime-a/src/function.pou",
                "runtime-a/src/nested/alpha.ST",
                "runtime-a/src/zeta.st",
            ]
        );
        assert_eq!(
            registry
                .files()
                .iter()
                .map(|file| file.id)
                .collect::<Vec<_>>(),
            vec![1, 2, 3]
        );

        let _ = std::fs::remove_dir_all(workspace);
    }

    #[test]
    fn config_mode_control_state_is_local_nonexecuting_and_source_backed() {
        let project = temp_dir("control-state");
        write_runtime(&project, "control-state-runtime");
        std::fs::create_dir_all(project.join("src")).expect("create source directory");
        std::fs::write(project.join("src/main.st"), "PROGRAM Main\nEND_PROGRAM\n")
            .expect("write main source");

        let state = build_config_mode_control_state(
            project.clone(),
            SmolStr::new("control-state-runtime"),
            "127.0.0.1:18080",
        )
        .expect("build config mode control state");
        assert_eq!(state.project_root.as_deref(), Some(project.as_path()));
        assert_eq!(state.resource_name, SmolStr::new("control-state-runtime"));
        assert_eq!(state.sources.files().len(), 1);
        assert!(!state.control_requires_auth);
        assert!(!state
            .debug_enabled
            .load(std::sync::atomic::Ordering::Relaxed));

        let settings = state.settings.lock().expect("settings lock");
        assert!(settings.web.enabled);
        assert!(!settings.discovery.enabled);
        assert!(!settings.mesh.enabled);
        drop(settings);

        drop(state);
        let _ = std::fs::remove_dir_all(project);
    }
}
