fn print_trust_banner(
    bundle: Option<&RuntimeBundle>,
    web_url: Option<&str>,
    simulation_enabled: bool,
    simulation_time_scale: u32,
    execution_backend_source: trust_runtime::execution_backend::ExecutionBackendSource,
) {
    crate::style::print_logo();
    outln!("Your PLC is running.");
    if let Some(warning) = simulation_warning_message(simulation_enabled, simulation_time_scale) {
        outln!("{}", style::warning(warning));
    }
    if let Some(bundle) = bundle {
        outln!("PLC name: {}", bundle.runtime.resource_name);
        outln!("Project: {}", bundle.root.display());
        outln!(
            "Execution backend: {} ({})",
            trust_runtime::execution_backend::EXECUTION_BACKEND,
            execution_backend_source.as_str()
        );
        outln!(
            "I/O drivers: {}",
            enabled_io_driver_names(bundle).join(", ")
        );
        outln!(
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
        outln!("Open: {web_url}");
    } else {
        outln!(
            "Execution backend: {} ({})",
            trust_runtime::execution_backend::EXECUTION_BACKEND,
            execution_backend_source.as_str()
        );
        outln!("Web UI: disabled");
    }
    outln!("Press Ctrl+C to stop.");
}

fn format_endpoint(endpoint: &ControlEndpoint) -> String {
    match endpoint {
        ControlEndpoint::Tcp(addr) => format!("tcp://{addr}"),
        #[cfg(unix)]
        ControlEndpoint::Unix(path) => format!("unix://{}", path.display()),
    }
}

fn load_sources(root: &Path) -> anyhow::Result<SourceRegistry> {
    load_source_registry(root, Some(root))
}

fn print_startup_summary(
    bundle: &RuntimeBundle,
    restart: RestartMode,
    endpoint: &ControlEndpoint,
    opcua_endpoint: Option<&str>,
    simulation_enabled: bool,
    simulation_time_scale: u32,
    execution_backend_source: trust_runtime::execution_backend::ExecutionBackendSource,
) {
    outln!("project folder: {}", bundle.root.display());
    outln!("PLC name: {}", bundle.runtime.resource_name);
    outln!("restart: {restart:?}");
    outln!(
        "mode: {} (time scale x{})",
        if simulation_enabled {
            "simulation"
        } else {
            "production"
        },
        simulation_time_scale
    );
    outln!(
        "cycle interval: {} ms",
        bundle.runtime.cycle_interval.as_millis()
    );
    outln!(
        "execution backend: {} ({})",
        trust_runtime::execution_backend::EXECUTION_BACKEND,
        execution_backend_source.as_str()
    );
    outln!("io drivers: {}", enabled_io_driver_names(bundle).join(", "));
    outln!("control mode: {:?}", bundle.runtime.control_mode);
    outln!(
        "debug: {}",
        if bundle.runtime.control_debug_enabled {
            "enabled"
        } else {
            "disabled"
        }
    );
    if let Some(token) = bundle.runtime.control_auth_token.as_ref() {
        outln!("control auth: set (len={})", token.len());
    } else {
        outln!("control auth: none");
    }
    outln!(
        "retain: {} {}",
        format_retain_mode(bundle.runtime.retain_mode),
        bundle
            .runtime
            .retain_path
            .as_ref()
            .map(|path| format!("({})", path.display()))
            .unwrap_or_default()
    );
    outln!(
        "retain save: {} ms",
        bundle.runtime.retain_save_interval.as_millis()
    );
    outln!(
        "watchdog: enabled={} timeout={} ms action={:?}",
        bundle.runtime.watchdog.enabled,
        bundle.runtime.watchdog.timeout.as_millis(),
        bundle.runtime.watchdog.action
    );
    outln!("fault policy: {:?}", bundle.runtime.fault_policy);
    outln!("control endpoint: {}", format_endpoint(endpoint));
    outln!(
        "web ui: {} ({})",
        if bundle.runtime.web.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.web.listen
    );
    outln!(
        "discovery: {} ({})",
        if bundle.runtime.discovery.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.discovery.service_name
    );
    outln!(
        "mesh: {} ({})",
        if bundle.runtime.mesh.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.mesh.listen
    );
    outln!(
        "opc ua: {} ({})",
        if bundle.runtime.opcua.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.opcua.listen
    );
    if let Some(endpoint) = opcua_endpoint {
        outln!("opc ua endpoint: {endpoint}");
    }
}
