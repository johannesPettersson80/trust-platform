use super::simulation_warning_message;
#[cfg(unix)]
use std::sync::atomic::{AtomicBool, Ordering};
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointQuality, SymbolDescriptor, SymbolFlag,
};
use trust_runtime::harness::{CompileSession, SourceFile, TestHarness};
use trust_runtime::value::{Duration, Value};

fn bundle_with_backend(
    backend: trust_runtime::execution_backend::ExecutionBackend,
) -> trust_runtime::config::RuntimeBundle {
    trust_runtime::config::RuntimeBundle {
        root: std::path::PathBuf::from("."),
        runtime: trust_runtime::config::RuntimeConfig {
            bundle_version: 1,
            resource_name: smol_str::SmolStr::new("RESOURCE"),
            cycle_interval: trust_runtime::value::Duration::from_millis(10),
            execution_backend: backend,
            execution_backend_source:
                trust_runtime::execution_backend::ExecutionBackendSource::Config,
            control_endpoint: smol_str::SmolStr::new("tcp://127.0.0.1:9000"),
            control_auth_token: Some(smol_str::SmolStr::new("secret")),
            control_debug_enabled: true,
            control_mode: trust_runtime::config::ControlMode::Debug,
            log_level: smol_str::SmolStr::new("info"),
            retain_mode: trust_runtime::watchdog::RetainMode::None,
            retain_path: None,
            retain_save_interval: trust_runtime::value::Duration::from_millis(1000),
            watchdog: trust_runtime::watchdog::WatchdogPolicy::default(),
            fault_policy: trust_runtime::watchdog::FaultPolicy::SafeHalt,
            web: trust_runtime::config::WebConfig {
                enabled: false,
                listen: smol_str::SmolStr::new("127.0.0.1:8080"),
                auth: trust_runtime::config::WebAuthMode::Local,
                tls: false,
            },
            tls: trust_runtime::config::TlsConfig {
                mode: trust_runtime::config::TlsMode::Disabled,
                cert_path: None,
                key_path: None,
                ca_path: None,
                require_remote: false,
            },
            deploy: trust_runtime::config::DeployConfig {
                require_signed: false,
                keyring_path: None,
            },
            discovery: trust_runtime::config::DiscoveryConfig {
                enabled: false,
                service_name: smol_str::SmolStr::new("truST"),
                advertise: false,
                interfaces: Vec::new(),
                host_group: None,
            },
            mesh: trust_runtime::config::MeshConfig {
                enabled: false,
                role: trust_runtime::config::MeshRole::Peer,
                listen: smol_str::SmolStr::new("0.0.0.0:5200"),
                connect: Vec::new(),
                tls: false,
                auth_token: None,
                publish: Vec::new(),
                subscribe: indexmap::IndexMap::new(),
                zenohd_version: smol_str::SmolStr::new("1.7.2"),
                plugin_versions: indexmap::IndexMap::new(),
            },
            runtime_cloud_profile: trust_runtime::config::RuntimeCloudProfile::Dev,
            runtime_cloud_wan_allow_write: Vec::new(),
            runtime_cloud_link_preferences: Vec::new(),
            realtime: trust_runtime::linux_rt::LinuxRtConfig::default(),
            openot: trust_runtime::config::OpenOtTelemetryConfig::default(),
            observability: trust_runtime::historian::HistorianConfig::default(),
            hmi_persistence: trust_runtime::hmi::HmiPersistenceConfig::default(),
            ads: trust_runtime::config::AdsRuntimeConfig::default(),
            ads_server: trust_runtime::ads::server::AdsServerRuntimeConfig::default(),
            opcua_client: trust_runtime::config::OpcUaClientRuntimeConfig::default(),
            opcua: trust_runtime::opcua::OpcUaRuntimeConfig::default(),
            tasks: None,
        },
        io: trust_runtime::config::IoConfig {
            drivers: Vec::new(),
            safe_state: trust_runtime::io::IoSafeState::default(),
        },
        ads: None,
        ads_config_hash: None,
        opcua_client: None,
        opcua_client_config_hash: None,
        simulation: None,
        bytecode: Vec::new(),
    }
}

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

#[test]
fn execution_backend_selection_defaults_to_vm() {
    let (backend, source) =
        super::resolve_execution_backend_selection(None, None).expect("resolve backend");
    assert_eq!(
        backend,
        trust_runtime::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        source,
        trust_runtime::execution_backend::ExecutionBackendSource::Default
    );
}

#[test]
fn execution_backend_selection_prefers_cli_override() {
    let (backend, source) =
        super::resolve_execution_backend_selection(None, Some(crate::cli::ExecutionBackendArg::Vm))
            .expect("resolve backend");
    assert_eq!(
        backend,
        trust_runtime::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        source,
        trust_runtime::execution_backend::ExecutionBackendSource::Flag
    );
}

#[test]
fn modbus_io_source_label_uses_register_direction() {
    let params: toml::Value = toml::toml! {
        address = "127.0.0.1:1502"
        input_start = 10
        output_start = 20
    }
    .into();
    let driver = trust_runtime::config::IoDriverConfig {
        name: smol_str::SmolStr::new("modbus-tcp"),
        params,
        enabled: true,
    };

    let input = trust_runtime::io::IoAddress::parse("%IX4.0").expect("input address");
    let output = trust_runtime::io::IoAddress::parse("%QX2.0").expect("output address");

    assert_eq!(
        trust_runtime::io::io_source_label_for_driver_address(&driver, &input).as_deref(),
        Some("Modbus 127.0.0.1:1502 · input reg 12")
    );
    assert_eq!(
        trust_runtime::io::io_source_label_for_driver_address(&driver, &output).as_deref(),
        Some("Modbus 127.0.0.1:1502 · output reg 21")
    );
}

#[test]
fn mqtt_io_source_label_uses_directional_topic() {
    let params: toml::Value = toml::toml! {
        topic_in = "trust/examples/mqtt/in"
        topic_out = "trust/examples/mqtt/out"
    }
    .into();
    let driver = trust_runtime::config::IoDriverConfig {
        name: smol_str::SmolStr::new("mqtt"),
        params,
        enabled: true,
    };

    let input = trust_runtime::io::IoAddress::parse("%IX0.0").expect("input address");
    let output = trust_runtime::io::IoAddress::parse("%QX0.0").expect("output address");

    assert_eq!(
        trust_runtime::io::io_source_label_for_driver_address(&driver, &input).as_deref(),
        Some("MQTT topic trust/examples/mqtt/in")
    );
    assert_eq!(
        trust_runtime::io::io_source_label_for_driver_address(&driver, &output).as_deref(),
        Some("MQTT topic trust/examples/mqtt/out")
    );
}

#[test]
fn execution_backend_selection_uses_bundle_when_cli_absent() {
    let bundle =
        bundle_with_backend(trust_runtime::execution_backend::ExecutionBackend::BytecodeVm);

    let (backend, source) =
        super::resolve_execution_backend_selection(Some(&bundle), None).expect("resolve backend");
    assert_eq!(
        backend,
        trust_runtime::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        source,
        trust_runtime::execution_backend::ExecutionBackendSource::Config
    );
}

#[test]
fn execution_backend_selection_cli_overrides_bundle() {
    let bundle =
        bundle_with_backend(trust_runtime::execution_backend::ExecutionBackend::BytecodeVm);
    let (backend, source) = super::resolve_execution_backend_selection(
        Some(&bundle),
        Some(crate::cli::ExecutionBackendArg::Vm),
    )
    .expect("resolve backend");
    assert_eq!(
        backend,
        trust_runtime::execution_backend::ExecutionBackend::BytecodeVm
    );
    assert_eq!(
        source,
        trust_runtime::execution_backend::ExecutionBackendSource::Flag
    );
}

#[cfg(unix)]
#[derive(Debug)]
struct FakeSignalSource {
    signal: Option<super::RuntimeShutdownSignal>,
}

#[cfg(unix)]
impl super::RuntimeSignalSource for FakeSignalSource {
    fn recv_shutdown_signal(&mut self) -> std::io::Result<super::RuntimeShutdownSignal> {
        self.signal
            .take()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::UnexpectedEof, "no fake signal"))
    }
}

#[cfg(unix)]
#[derive(Debug, Default)]
struct RecordingShutdownTarget {
    called: AtomicBool,
}

#[cfg(unix)]
impl super::RuntimeShutdownTarget for RecordingShutdownTarget {
    fn request_shutdown(&self) {
        self.called.store(true, Ordering::SeqCst);
    }
}

#[cfg(unix)]
#[test]
fn signal_abstraction_requests_regular_runtime_shutdown() {
    for signal in [
        super::RuntimeShutdownSignal::Interrupt,
        super::RuntimeShutdownSignal::Terminate,
    ] {
        let mut source = FakeSignalSource {
            signal: Some(signal),
        };
        let target = RecordingShutdownTarget::default();

        let observed =
            super::request_shutdown_from_signal(&mut source, &target).expect("signal shutdown");

        assert_eq!(observed, signal);
        assert!(
            target.called.load(Ordering::SeqCst),
            "signal shutdown must use the ordinary stop target"
        );
    }
}

#[cfg(unix)]
#[test]
fn unix_signal_mapping_covers_sigint_and_sigterm() {
    assert_eq!(
        super::map_runtime_shutdown_signal(signal_hook::consts::signal::SIGINT),
        Some(super::RuntimeShutdownSignal::Interrupt)
    );
    assert_eq!(
        super::map_runtime_shutdown_signal(signal_hook::consts::signal::SIGTERM),
        Some(super::RuntimeShutdownSignal::Terminate)
    );
}

#[cfg(unix)]
#[test]
fn unix_signal_mapping_rejects_unreviewed_shutdown_signals() {
    for signal in [
        signal_hook::consts::signal::SIGHUP,
        signal_hook::consts::signal::SIGUSR1,
    ] {
        assert_eq!(super::map_runtime_shutdown_signal(signal), None);
    }
}

#[test]
fn startup_retain_load_respects_restart_mode() {
    let source = r#"
VAR_GLOBAL RETAIN
    r : INT := 1;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let mut path = std::env::temp_dir();
    path.push(format!(
        "trust_runtime_startup_retain_{}.bin",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&path);

    let mut saver = TestHarness::from_source(source).expect("build saver runtime");
    saver.runtime_mut().set_retain_store(
        Some(Box::new(trust_runtime::retain::FileRetainStore::new(&path))),
        Some(Duration::from_millis(1)),
    );
    saver.set_input("r", Value::Int(42));
    saver.runtime_mut().mark_retain_dirty();
    saver
        .runtime_mut()
        .save_retain_store()
        .expect("save retain");

    let mut cold = TestHarness::from_source(source)
        .expect("build cold runtime")
        .into_runtime();
    cold.set_retain_store(
        Some(Box::new(trust_runtime::retain::FileRetainStore::new(&path))),
        Some(Duration::from_millis(1)),
    );
    cold.restart(trust_runtime::RestartMode::Cold)
        .expect("cold restart");
    super::load_startup_retain(&mut cold, trust_runtime::RestartMode::Cold)
        .expect("cold startup retain");
    assert_eq!(cold.storage().get_global("r"), Some(&Value::Int(1)));

    let mut warm = TestHarness::from_source(source)
        .expect("build warm runtime")
        .into_runtime();
    warm.set_retain_store(
        Some(Box::new(trust_runtime::retain::FileRetainStore::new(&path))),
        Some(Duration::from_millis(1)),
    );
    warm.restart(trust_runtime::RestartMode::Warm)
        .expect("warm restart");
    super::load_startup_retain(&mut warm, trust_runtime::RestartMode::Warm)
        .expect("warm startup retain");
    assert_eq!(warm.storage().get_global("r"), Some(&Value::Int(42)));

    let _ = std::fs::remove_file(path);
}

#[test]
fn ads_runtime_start_spawns_worker_and_scan_applies_mock_data() {
    let source = r#"
TYPE
    ADS_QUALITY : (Stale := 0, Good := 1, Error := 2);
END_TYPE

VAR_GLOBAL
    line1_temp : REAL;
    line1_temp_quality : ADS_QUALITY := Stale;
END_VAR

PROGRAM Main
END_PROGRAM
"#;
    let session = CompileSession::from_sources(vec![SourceFile::with_path(
        "src/generated/ads_generated.st",
        source,
    )]);
    let mut runtime = session.build_runtime().expect("build runtime");
    let bytecode = session.build_bytecode_bytes().expect("build bytecode");
    runtime
        .apply_bytecode_bytes(&bytecode, Some(&smol_str::SmolStr::new("RESOURCE")))
        .expect("apply bytecode");
    let mut bundle =
        bundle_with_backend(trust_runtime::execution_backend::ExecutionBackend::BytecodeVm);
    bundle.runtime.ads.enabled = true;
    bundle.runtime.ads.worker_tick_interval = trust_runtime::value::Duration::from_millis(1);
    bundle.ads_config_hash = Some("sha256:test-ads-config".to_string());
    bundle.ads = Some(
        trust_runtime::ads::parse_ads_toml(
            r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
"#,
        )
        .expect("parse ADS config"),
    );

    super::start_ads_runtime_with_factory(&mut runtime, &bundle, |_connection| {
        let mut transport = trust_runtime::ads::MockAdsTransport::new(vec![SymbolDescriptor::new(
            "MAIN.Temperature",
            AdsDataTypeDescriptor::scalar("REAL", IecDataType::Real),
            0x4020,
            0,
            4,
        )
        .with_flag(SymbolFlag::Read)
        .with_flag(SymbolFlag::Write)]);
        transport.set_value("line1_temp", Value::Real(42.5), PointQuality::good(10));
        Ok(transport)
    })
    .expect("start ADS runtime");

    assert_eq!(runtime.ads_connection_count(), 1);
    for _ in 0..50 {
        std::thread::sleep(std::time::Duration::from_millis(2));
        runtime.execute_cycle().expect("execute cycle");
        if runtime.storage().get_global("line1_temp") == Some(&Value::Real(42.5)) {
            break;
        }
    }

    assert_eq!(
        runtime.storage().get_global("line1_temp"),
        Some(&Value::Real(42.5))
    );
    assert_enum_variant(
        runtime
            .storage()
            .get_global("line1_temp_quality")
            .expect("quality global"),
        "Good",
    );
    let status = runtime.ads_status_report();
    assert_eq!(
        status.overall,
        trust_runtime::ads::diagnostics::AdsStatusOverall::Healthy
    );
    assert_eq!(
        status.deployed_ads_config_hash.as_deref(),
        Some("sha256:test-ads-config")
    );
    assert_eq!(status.connections.len(), 1);
    assert_eq!(status.connections[0].name, "line1");
    assert_eq!(status.connections[0].degraded_points, 0);
    assert_eq!(
        status.connections[0].state,
        trust_runtime::ads::diagnostics::AdsConnectionStatusState::Connected
    );
    runtime.shutdown_ads().expect("shutdown ADS worker");
}

#[test]
fn project_runtime_load_includes_local_dependencies() {
    let root = unique_temp_dir("run-project-deps");
    let project = root.join("project");
    let dependency = root.join("dep");
    std::fs::create_dir_all(project.join("src")).expect("create project src");
    std::fs::create_dir_all(dependency.join("src")).expect("create dependency src");

    std::fs::write(
        project.join("trust-lsp.toml"),
        r#"[project]
include_paths = ["src"]

[dependencies]
DepLib = { path = "../dep" }
"#,
    )
    .expect("write project manifest");
    std::fs::write(
        dependency.join("trust-lsp.toml"),
        r#"[package]
version = "1.0.0"

[project]
include_paths = ["src"]
"#,
    )
    .expect("write dependency manifest");
    std::fs::write(
        dependency.join("src").join("shared.st"),
        r#"TYPE E_SHARED_STATE : (Idle := 0, Ready := 1) END_TYPE
"#,
    )
    .expect("write dependency source");
    std::fs::write(
        project.join("src").join("main.st"),
        r#"PROGRAM Main
VAR
    state : E_SHARED_STATE := E_SHARED_STATE#Ready;
END_VAR
END_PROGRAM
"#,
    )
    .expect("write project source");
    std::fs::write(project.join("program.stbc"), []).expect("write placeholder bytecode");
    std::fs::write(
        project.join("io.toml"),
        r#"[io]
driver = "simulated"
params = {}
"#,
    )
    .expect("write io.toml");
    std::fs::write(
        project.join("runtime.toml"),
        r#"[bundle]
version = 1

[resource]
name = "DependencyRun"
cycle_interval_ms = 100

[runtime]
execution_backend = "vm"

[runtime.control]
endpoint = "tcp://127.0.0.1:0"
mode = "production"
auth_token = "test-token"
debug_enabled = false

[runtime.web]
enabled = false
listen = "127.0.0.1:8080"
auth = "local"
tls = false

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = "halt"

[runtime.fault]
policy = "halt"
"#,
    )
    .expect("write runtime.toml");

    let loaded = super::load_runtime(Some(project), None, None)
        .expect("project runtime load should compile sources and local dependencies");

    assert!(
        loaded
            .sources
            .files()
            .iter()
            .any(|file| file.path.ends_with("shared.st")),
        "dependency source must be included in project run source registry"
    );
    let _ = std::fs::remove_dir_all(root);
}

fn unique_temp_dir(name: &str) -> std::path::PathBuf {
    let mut path = std::env::temp_dir();
    path.push(format!(
        "trust_runtime_{name}_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos()
    ));
    path
}

fn assert_enum_variant(value: &Value, expected: &str) {
    let Value::Enum(value) = value else {
        panic!("expected enum value, got {value:?}");
    };
    assert_eq!(value.variant_name().as_str(), expected);
}
