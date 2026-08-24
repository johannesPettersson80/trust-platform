use super::*;

fn invalid_text(error: RuntimeError) -> String {
    error.to_string()
}

#[test]
fn web_auth_mode_accepts_complete_trimmed_case_insensitive_vocabulary() {
    for (text, expected) in [
        ("local", WebAuthMode::Local),
        (" LOCAL ", WebAuthMode::Local),
        ("\ttoken\n", WebAuthMode::Token),
        ("ToKeN", WebAuthMode::Token),
    ] {
        assert_eq!(WebAuthMode::parse(text).expect("web auth"), expected);
    }
}

#[test]
fn web_auth_mode_rejects_every_near_match_with_owning_key() {
    for text in ["", "loc", "tokens", "local auth", "token-mode", "local\0"] {
        let error = WebAuthMode::parse(text).expect_err("invalid web auth");
        let message = invalid_text(error);
        assert!(message.contains("runtime.web.auth"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn tls_mode_accepts_canonical_and_compatibility_spellings() {
    for (text, expected) in [
        ("disabled", TlsMode::Disabled),
        (" DISABLED ", TlsMode::Disabled),
        ("self-managed", TlsMode::SelfManaged),
        ("SELF_MANAGED", TlsMode::SelfManaged),
        ("\tprovisioned\n", TlsMode::Provisioned),
    ] {
        assert_eq!(TlsMode::parse(text).expect("TLS mode"), expected);
    }
}

#[test]
fn tls_mode_rejects_near_matches_with_owning_key() {
    for text in [
        "",
        "disable",
        "self managed",
        "self--managed",
        "provision",
        "enabled",
        "disabled\0",
    ] {
        let message = invalid_text(TlsMode::parse(text).expect_err("invalid TLS mode"));
        assert!(message.contains("runtime.tls.mode"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn tls_enabled_matches_only_non_disabled_modes() {
    assert!(!TlsMode::Disabled.enabled());
    assert!(TlsMode::SelfManaged.enabled());
    assert!(TlsMode::Provisioned.enabled());
}

#[test]
fn mesh_role_round_trips_complete_vocabulary() {
    for (text, expected, canonical) in [
        ("peer", MeshRole::Peer, "peer"),
        (" CLIENT ", MeshRole::Client, "client"),
        ("\tRoUtEr\n", MeshRole::Router, "router"),
    ] {
        let parsed = MeshRole::parse(text).expect("mesh role");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), canonical);
    }
}

#[test]
fn mesh_role_rejects_near_matches_with_owning_key() {
    for text in ["", "server", "route", "client-mode", "peer role", "peer\0"] {
        let message = invalid_text(MeshRole::parse(text).expect_err("invalid mesh role"));
        assert!(message.contains("runtime.mesh.role"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn runtime_cloud_profile_round_trips_complete_vocabulary() {
    for (text, expected, canonical) in [
        ("dev", RuntimeCloudProfile::Dev, "dev"),
        (" PLANT ", RuntimeCloudProfile::Plant, "plant"),
        ("\tWaN\n", RuntimeCloudProfile::Wan, "wan"),
    ] {
        let parsed = RuntimeCloudProfile::parse(text).expect("cloud profile");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), canonical);
    }
}

#[test]
fn runtime_cloud_profile_rejects_near_matches_with_owning_key() {
    for text in ["", "development", "factory", "wide-area", "wan profile", "wan\0"] {
        let message =
            invalid_text(RuntimeCloudProfile::parse(text).expect_err("invalid cloud profile"));
        assert!(message.contains("runtime.cloud.profile"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn runtime_cloud_secure_transport_requirement_is_exact() {
    assert!(!RuntimeCloudProfile::Dev.requires_secure_transport());
    assert!(RuntimeCloudProfile::Plant.requires_secure_transport());
    assert!(RuntimeCloudProfile::Wan.requires_secure_transport());
}

#[test]
fn preferred_transport_round_trips_complete_vocabulary() {
    for (text, expected, canonical) in [
        ("realtime", RuntimeCloudPreferredTransport::Realtime, "realtime"),
        (" ZENOH ", RuntimeCloudPreferredTransport::Zenoh, "zenoh"),
        ("mesh", RuntimeCloudPreferredTransport::Mesh, "mesh"),
        ("mqtt", RuntimeCloudPreferredTransport::Mqtt, "mqtt"),
        (
            "modbus-tcp",
            RuntimeCloudPreferredTransport::ModbusTcp,
            "modbus-tcp",
        ),
        (
            "MODBUS_TCP",
            RuntimeCloudPreferredTransport::ModbusTcp,
            "modbus-tcp",
        ),
        ("opcua", RuntimeCloudPreferredTransport::OpcUa, "opcua"),
        (
            "discovery",
            RuntimeCloudPreferredTransport::Discovery,
            "discovery",
        ),
        ("web", RuntimeCloudPreferredTransport::Web, "web"),
    ] {
        let parsed = RuntimeCloudPreferredTransport::parse(text).expect("transport");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), canonical);
    }
}

#[test]
fn preferred_transport_rejects_near_matches_with_owning_key() {
    for text in [
        "",
        "real-time",
        "modbus tcp",
        "opc-ua",
        "http",
        "auto",
        "web\0",
    ] {
        let message = invalid_text(
            RuntimeCloudPreferredTransport::parse(text).expect_err("invalid transport"),
        );
        assert!(
            message.contains("runtime.cloud.links.transports[].transport"),
            "text={text:?}"
        );
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn openot_fence_mode_round_trips_complete_vocabulary() {
    for (text, expected, canonical) in [
        ("fenced", OpenOtTelemetryFenceMode::Fenced, "fenced"),
        (
            " UNFENCED ",
            OpenOtTelemetryFenceMode::Unfenced,
            "unfenced",
        ),
    ] {
        let parsed = OpenOtTelemetryFenceMode::parse(text).expect("fence mode");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), canonical);
    }
}

#[test]
fn openot_fence_mode_rejects_near_matches_with_owning_key() {
    for text in ["", "fence", "un-fenced", "none", "fenced mode", "fenced\0"] {
        let message = invalid_text(
            OpenOtTelemetryFenceMode::parse(text).expect_err("invalid fence mode"),
        );
        assert!(message.contains("runtime.openot.fence_mode"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn openot_source_round_trips_complete_vocabulary() {
    for (text, expected, canonical) in [
        ("heartbeat", OpenOtTelemetrySource::Heartbeat, "heartbeat"),
        (" ST-FB ", OpenOtTelemetrySource::StFb, "st-fb"),
    ] {
        let parsed = OpenOtTelemetrySource::parse(text).expect("OpenOT source");
        assert_eq!(parsed, expected);
        assert_eq!(parsed.as_str(), canonical);
    }
}

#[test]
fn openot_source_rejects_near_matches_with_owning_key() {
    for text in ["", "heart-beat", "st_fb", "stfb", "automatic", "st-fb\0"] {
        let message =
            invalid_text(OpenOtTelemetrySource::parse(text).expect_err("invalid source"));
        assert!(message.contains("runtime.openot.source"), "text={text:?}");
        assert!(message.contains(text), "text={text:?}");
    }
}

#[test]
fn openot_default_is_inert_fenced_heartbeat_configuration() {
    let config = OpenOtTelemetryConfig::default();
    assert!(!config.enabled);
    assert!(config.path.as_os_str().is_empty());
    assert_eq!(config.capacity, 4096);
    assert_eq!(config.fence_mode, OpenOtTelemetryFenceMode::Fenced);
    assert!(!config.allow_unfenced_for_proof);
    assert_eq!(config.source, OpenOtTelemetrySource::Heartbeat);
    assert_eq!(config.producer_instance, None);
    assert!(config.producer_instances.is_empty());
}

#[test]
fn ads_runtime_default_is_disabled_with_exact_sidecar_and_tick() {
    let config = AdsRuntimeConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.config_path, PathBuf::from("ads.toml"));
    assert_eq!(config.worker_tick_interval, Duration::from_millis(20));
}

#[test]
fn opcua_client_default_is_disabled_with_exact_sidecar_and_poll() {
    let config = OpcUaClientRuntimeConfig::default();
    assert!(!config.enabled);
    assert_eq!(config.config_path, PathBuf::from("opcua_client.toml"));
    assert_eq!(config.poll_interval, Duration::from_millis(250));
}

#[test]
fn wan_allow_rule_preserves_exact_action_and_target() {
    let rule = RuntimeCloudWanAllowRule {
        action: "write".into(),
        target: "cell/line1".into(),
    };
    let copy = rule.clone();
    assert_eq!(copy, rule);
    assert_eq!(copy.action, "write");
    assert_eq!(copy.target, "cell/line1");
}

#[test]
fn link_preference_rule_preserves_exact_endpoints_and_transport() {
    let rule = RuntimeCloudLinkPreferenceRule {
        source: "edge-a".into(),
        target: "edge-b".into(),
        transport: RuntimeCloudPreferredTransport::Zenoh,
    };
    let copy = rule.clone();
    assert_eq!(copy, rule);
    assert_eq!(copy.source, "edge-a");
    assert_eq!(copy.target, "edge-b");
    assert_eq!(copy.transport, RuntimeCloudPreferredTransport::Zenoh);
}

#[test]
fn openot_config_clone_owns_path_and_producer_lists() {
    let mut original = OpenOtTelemetryConfig {
        enabled: true,
        path: PathBuf::from("telemetry.jsonl"),
        capacity: 7,
        fence_mode: OpenOtTelemetryFenceMode::Unfenced,
        allow_unfenced_for_proof: true,
        source: OpenOtTelemetrySource::StFb,
        producer_instance: Some("Plant.Telemetry".into()),
        producer_instances: vec!["Plant.Telemetry".into(), "Line.Telemetry".into()],
    };
    let copy = original.clone();

    original.path.push("changed");
    original.producer_instance = None;
    original.producer_instances.clear();
    assert_eq!(copy.path, PathBuf::from("telemetry.jsonl"));
    assert_eq!(
        copy.producer_instance.as_deref(),
        Some("Plant.Telemetry")
    );
    assert_eq!(copy.producer_instances.len(), 2);
}

#[test]
fn tls_config_clone_owns_all_optional_paths() {
    let mut original = TlsConfig {
        mode: TlsMode::Provisioned,
        cert_path: Some(PathBuf::from("cert.pem")),
        key_path: Some(PathBuf::from("key.pem")),
        ca_path: Some(PathBuf::from("ca.pem")),
        require_remote: true,
    };
    let copy = original.clone();
    original.cert_path.as_mut().expect("cert").push("changed");
    original.key_path = None;
    original.ca_path = None;

    assert_eq!(copy.mode, TlsMode::Provisioned);
    assert_eq!(copy.cert_path, Some(PathBuf::from("cert.pem")));
    assert_eq!(copy.key_path, Some(PathBuf::from("key.pem")));
    assert_eq!(copy.ca_path, Some(PathBuf::from("ca.pem")));
    assert!(copy.require_remote);
}

#[test]
fn discovery_config_clone_owns_interface_and_host_group_values() {
    let mut original = DiscoveryConfig {
        enabled: true,
        service_name: "runtime".into(),
        advertise: true,
        interfaces: vec!["eth0".into(), "wlan0".into()],
        host_group: Some("plant".into()),
    };
    let copy = original.clone();
    original.interfaces.clear();
    original.host_group = None;

    assert_eq!(copy.service_name, "runtime");
    assert_eq!(
        copy.interfaces
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["eth0", "wlan0"]
    );
    assert_eq!(copy.host_group.as_deref(), Some("plant"));
}

#[test]
fn mesh_config_clone_owns_connection_and_subscription_maps() {
    let mut original = MeshConfig {
        enabled: true,
        role: MeshRole::Router,
        listen: "0.0.0.0:7447".into(),
        connect: vec!["tcp/edge-a:7447".into()],
        tls: true,
        auth_token: Some("secret".into()),
        publish: vec!["plant/**".into()],
        subscribe: IndexMap::from([("remote/**".into(), "local/".into())]),
        zenohd_version: "1.0.0".into(),
        plugin_versions: IndexMap::from([("storage".into(), "1.0.0".into())]),
    };
    let copy = original.clone();
    original.connect.clear();
    original.publish.clear();
    original.subscribe.clear();
    original.plugin_versions.clear();

    assert_eq!(copy.role, MeshRole::Router);
    assert_eq!(copy.connect.len(), 1);
    assert_eq!(copy.publish.len(), 1);
    assert_eq!(copy.subscribe.len(), 1);
    assert_eq!(copy.plugin_versions.len(), 1);
}

#[test]
fn io_driver_config_equality_includes_name_parameters_and_enabled_state() {
    let base = IoDriverConfig {
        name: "modbus".into(),
        params: toml::Value::Table(toml::Table::from_iter([(
            "endpoint".into(),
            toml::Value::String("127.0.0.1:502".into()),
        )])),
        enabled: true,
    };
    assert_eq!(base, base.clone());

    let mut disabled = base.clone();
    disabled.enabled = false;
    assert_ne!(base, disabled);
}

#[test]
fn task_override_clone_owns_name_programs_and_single_trigger() {
    let mut original = TaskOverride {
        name: "Fast".into(),
        interval: Duration::from_millis(10),
        priority: 1,
        programs: vec!["Main".into(), "Safety".into()],
        single: Some("run_once".into()),
    };
    let copy = original.clone();
    original.programs.clear();
    original.single = None;

    assert_eq!(copy.name, "Fast");
    assert_eq!(copy.interval, Duration::from_millis(10));
    assert_eq!(copy.priority, 1);
    assert_eq!(
        copy.programs
            .iter()
            .map(SmolStr::as_str)
            .collect::<Vec<_>>(),
        ["Main", "Safety"]
    );
    assert_eq!(copy.single.as_deref(), Some("run_once"));
}
