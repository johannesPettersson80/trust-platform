use super::simulation_warning_message;
use trust_ads_core::{
    AdsDataTypeDescriptor, IecDataType, PointQuality, SymbolDescriptor, SymbolFlag,
};
use trust_runtime::harness::{CompileSession, SourceFile};
use trust_runtime::value::Value;

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

fn assert_enum_variant(value: &Value, expected: &str) {
    let Value::Enum(value) = value else {
        panic!("expected enum value, got {value:?}");
    };
    assert_eq!(value.variant_name().as_str(), expected);
}
