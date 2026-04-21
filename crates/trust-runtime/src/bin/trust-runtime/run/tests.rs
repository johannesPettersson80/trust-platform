use super::simulation_warning_message;

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
            observability: trust_runtime::historian::HistorianConfig::default(),
            opcua: trust_runtime::opcua::OpcUaRuntimeConfig::default(),
            tasks: None,
        },
        io: trust_runtime::config::IoConfig {
            drivers: Vec::new(),
            safe_state: trust_runtime::io::IoSafeState::default(),
        },
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
