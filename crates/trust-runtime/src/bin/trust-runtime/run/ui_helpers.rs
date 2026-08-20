fn print_trust_banner(
    bundle: Option<&RuntimeBundle>,
    web_url: Option<&str>,
    simulation_enabled: bool,
    simulation_time_scale: u32,
    scaffold: Option<&trust_runtime::hmi::HmiScaffoldSummary>,
    execution_backend: trust_runtime::execution_backend::ExecutionBackend,
    execution_backend_source: trust_runtime::execution_backend::ExecutionBackendSource,
) {
    crate::style::print_logo();
    println!("Your PLC is running.");
    if let Some(warning) = simulation_warning_message(simulation_enabled, simulation_time_scale) {
        println!("{}", style::warning(warning));
    }
    if let Some(bundle) = bundle {
        println!("PLC name: {}", bundle.runtime.resource_name);
        println!("Project: {}", bundle.root.display());
        println!(
            "Execution backend: {} ({})",
            execution_backend.as_str(),
            execution_backend_source.as_str()
        );
        println!(
            "I/O drivers: {}",
            enabled_io_driver_names(bundle).join(", ")
        );
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
        let page_count = scaffold
            .map(|summary| {
                summary
                    .files
                    .iter()
                    .filter(|entry| entry.path.ends_with(".toml") && entry.path != "_config.toml")
                    .count()
            })
            .unwrap_or(0);
        if page_count > 0 {
            println!(
                "HMI ready: {web_url}/hmi ({page_count} pages scaffolded, edit mode available)"
            );
        } else {
            println!("HMI ready: {web_url}/hmi");
        }
    } else {
        println!(
            "Execution backend: {} ({})",
            execution_backend.as_str(),
            execution_backend_source.as_str()
        );
        println!("Web UI: disabled");
    }
    println!("Press Ctrl+C to stop.");
}

fn auto_scaffold_hmi_update(
    bundle: &RuntimeBundle,
    runtime: &Runtime,
    sources: &SourceRegistry,
) -> Option<trust_runtime::hmi::HmiScaffoldSummary> {
    let source_refs = sources
        .files()
        .iter()
        .map(|file| HmiSourceRef {
            path: file.path.as_path(),
            text: file.text.as_str(),
        })
        .collect::<Vec<_>>();
    if source_refs.is_empty() {
        return None;
    }
    let metadata = runtime.metadata_snapshot();
    let snapshot = trust_runtime::debug::DebugSnapshot {
        storage: runtime.storage().clone(),
        now: runtime.current_time(),
    };
    match trust_runtime::hmi::scaffold_hmi_dir_with_sources_mode(
        bundle.root.as_path(),
        &metadata,
        Some(&snapshot),
        &source_refs,
        "industrial",
        HmiScaffoldMode::Update,
        false,
    ) {
        Ok(summary) => Some(summary),
        Err(err) => {
            eprintln!(
                "{}",
                style::warning(format!(
                    "Warning: failed to update HMI scaffold automatically: {err}"
                ))
            );
            None
        }
    }
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
    execution_backend_selection: (
        trust_runtime::execution_backend::ExecutionBackend,
        trust_runtime::execution_backend::ExecutionBackendSource,
    ),
) {
    let (execution_backend, execution_backend_source) = execution_backend_selection;
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
    println!(
        "execution backend: {} ({})",
        execution_backend.as_str(),
        execution_backend_source.as_str()
    );
    println!("io drivers: {}", enabled_io_driver_names(bundle).join(", "));
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
    println!(
        "opc ua: {} ({})",
        if bundle.runtime.opcua.enabled {
            "enabled"
        } else {
            "disabled"
        },
        bundle.runtime.opcua.listen
    );
    if let Some(endpoint) = opcua_endpoint {
        println!("opc ua endpoint: {endpoint}");
    }
}
