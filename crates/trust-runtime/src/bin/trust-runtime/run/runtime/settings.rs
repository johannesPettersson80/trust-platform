fn build_runtime_settings(
    bundle: Option<&RuntimeBundle>,
    ide_shell_mode: bool,
    default_watchdog: trust_runtime::watchdog::WatchdogPolicy,
    default_fault: trust_runtime::watchdog::FaultPolicy,
    simulation: &SimulationPlan,
    execution_backend: trust_runtime::execution_backend::ExecutionBackend,
    execution_backend_source: trust_runtime::execution_backend::ExecutionBackendSource,
) -> RuntimeSettings {
    let mut settings = if let Some(bundle) = bundle {
        RuntimeSettings::new(
            bundle.runtime.cycle_interval,
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
                tls: bundle.runtime.web.tls,
            },
            DiscoverySettings {
                enabled: bundle.runtime.discovery.enabled,
                service_name: bundle.runtime.discovery.service_name.clone(),
                advertise: bundle.runtime.discovery.advertise,
                interfaces: bundle.runtime.discovery.interfaces.clone(),
                host_group: bundle.runtime.discovery.host_group.clone(),
            },
            MeshSettings {
                enabled: bundle.runtime.mesh.enabled,
                role: bundle.runtime.mesh.role,
                listen: bundle.runtime.mesh.listen.clone(),
                connect: bundle.runtime.mesh.connect.clone(),
                tls: bundle.runtime.mesh.tls,
                auth_token: bundle.runtime.mesh.auth_token.clone(),
                publish: bundle.runtime.mesh.publish.clone(),
                subscribe: bundle.runtime.mesh.subscribe.clone(),
                zenohd_version: bundle.runtime.mesh.zenohd_version.clone(),
                plugin_versions: bundle.runtime.mesh.plugin_versions.clone(),
            },
            SimulationSettings {
                enabled: simulation.enabled,
                time_scale: simulation.time_scale,
                mode_label: SmolStr::new(if simulation.enabled {
                    "simulation"
                } else {
                    "production"
                }),
                warning: SmolStr::new(simulation.warning.clone()),
            },
        )
    } else {
        RuntimeSettings::new(
            trust_runtime::value::Duration::from_millis(10),
            BaseSettings {
                log_level: SmolStr::new("info"),
                watchdog: default_watchdog,
                fault_policy: default_fault,
                retain_mode: trust_runtime::watchdog::RetainMode::None,
                retain_save_interval: None,
            },
            WebSettings {
                enabled: ide_shell_mode,
                listen: SmolStr::new("127.0.0.1:8082"),
                auth: SmolStr::new("local"),
                tls: false,
            },
            DiscoverySettings {
                enabled: false,
                service_name: SmolStr::new("truST"),
                advertise: false,
                interfaces: Vec::new(),
                host_group: None,
            },
            MeshSettings {
                enabled: false,
                role: trust_runtime::config::MeshRole::Peer,
                listen: SmolStr::new("0.0.0.0:5200"),
                connect: Vec::new(),
                tls: false,
                auth_token: None,
                publish: Vec::new(),
                subscribe: indexmap::IndexMap::new(),
                zenohd_version: SmolStr::new("1.7.2"),
                plugin_versions: indexmap::IndexMap::new(),
            },
            SimulationSettings {
                enabled: simulation.enabled,
                time_scale: simulation.time_scale,
                mode_label: SmolStr::new(if simulation.enabled {
                    "simulation"
                } else {
                    "production"
                }),
                warning: SmolStr::new(simulation.warning.clone()),
            },
        )
    };

    if let Some(bundle) = bundle {
        settings.opcua = OpcUaSettings {
            enabled: bundle.runtime.opcua.enabled,
            listen: bundle.runtime.opcua.listen.clone(),
            endpoint_path: bundle.runtime.opcua.endpoint_path.clone(),
            namespace_uri: bundle.runtime.opcua.namespace_uri.clone(),
            publish_interval_ms: bundle.runtime.opcua.publish_interval_ms,
            max_nodes: bundle.runtime.opcua.max_nodes,
            expose: bundle.runtime.opcua.expose.clone(),
            security_policy: SmolStr::new(bundle.runtime.opcua.security.policy.as_config_value()),
            security_mode: SmolStr::new(bundle.runtime.opcua.security.mode.as_config_value()),
            allow_anonymous: bundle.runtime.opcua.security.allow_anonymous,
            username_set: bundle.runtime.opcua.username.is_some(),
        };
        settings.runtime_cloud.profile = bundle.runtime.runtime_cloud_profile;
        settings.runtime_cloud.wan_allow_write =
            bundle.runtime.runtime_cloud_wan_allow_write.clone();
        settings.runtime_cloud.link_preferences =
            bundle.runtime.runtime_cloud_link_preferences.clone();
        settings.realtime = bundle.runtime.realtime.clone();
    }
    settings.execution_backend = execution_backend;
    settings.execution_backend_source = execution_backend_source;

    settings
}

fn resolve_web_config(bundle: Option<&RuntimeBundle>, settings: &RuntimeSettings) -> WebConfig {
    if let Some(bundle) = bundle {
        return bundle.runtime.web.clone();
    }

    let auth = if settings.web.auth.eq_ignore_ascii_case("token") {
        WebAuthMode::Token
    } else {
        WebAuthMode::Local
    };
    WebConfig {
        enabled: settings.web.enabled,
        listen: settings.web.listen.clone(),
        auth,
        tls: settings.web.tls,
    }
}

fn control_auth_token_value(bundle: Option<&RuntimeBundle>) -> Option<String> {
    bundle
        .and_then(|bundle| bundle.runtime.control_auth_token.as_ref())
        .map(|token| token.to_string())
}
