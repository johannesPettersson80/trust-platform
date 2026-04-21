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
    execution_backend: Option<crate::cli::ExecutionBackendArg>,
) -> anyhow::Result<()> {
    let restart_mode = parse_restart_mode(&restart)?;
    let LoadedRuntime {
        bundle,
        mut runtime,
        sources,
        ide_shell_mode,
    } = load_runtime(project, config, runtime_root)?;
    let (selected_execution_backend, selected_execution_backend_source) =
        resolve_execution_backend_selection(bundle.as_ref(), execution_backend)?;

    let simulation = build_simulation_plan(bundle.as_ref(), simulation, time_scale)?;
    let SimulationPlan {
        enabled: simulation_enabled,
        time_scale: simulation_time_scale,
        warning: simulation_warning,
        controller: simulation_controller,
    } = simulation;

    let debug = runtime.enable_debug();
    let metrics = Arc::new(Mutex::new(RuntimeMetrics::new()));
    if let Ok(mut guard) = metrics.lock() {
        guard.set_execution_backend(selected_execution_backend);
    }
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
        apply_bundle_runtime_overrides(&mut runtime, bundle)?;
    }
    runtime
        .set_execution_backend(selected_execution_backend)
        .map_err(|err| anyhow::anyhow!("{err}"))?;

    runtime.restart(restart_mode)?;
    runtime.load_retain_store()?;

    let startup_hmi_scaffold = bundle
        .as_ref()
        .and_then(|bundle| auto_scaffold_hmi_update(bundle, &runtime, &sources));

    let logger = RuntimeLogger::new(match &bundle {
        Some(bundle) => LogLevel::parse(bundle.runtime.log_level.as_str()),
        None => LogLevel::Info,
    });
    logger.log(
        LogLevel::Info,
        "execution_backend_selected",
        json!({
            "backend": selected_execution_backend.as_str(),
            "source": selected_execution_backend_source.as_str(),
        }),
    );

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
    let realtime_config = bundle
        .as_ref()
        .map(|bundle| bundle.runtime.realtime.clone())
        .unwrap_or_default();
    let realtime_status = Arc::new(Mutex::new(LinuxRtRuntimeStatus::from_config(
        realtime_config.clone(),
    )));

    let control_endpoint = parse_control_endpoint(bundle.as_ref())?;
    let tls_materials = if let Some(bundle) = bundle.as_ref() {
        load_tls_materials(&bundle.runtime.tls, Some(bundle.root.as_path()))?.map(Arc::new)
    } else {
        None
    };
    ensure_control_auth_requirements(&control_endpoint, bundle.as_ref(), ide_shell_mode)?;

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
    if realtime_config.enabled {
        runner = runner.with_thread_init_hook(make_thread_init_hook(
            realtime_config.clone(),
            realtime_status.clone(),
        ));
    }
    if let Some(simulation) = simulation_controller {
        runner = runner.with_simulation(simulation);
    }
    let mut handle = runner.spawn("trust-runtime")?;
    if let Ok(status) = realtime_status.lock() {
        logger.log(
            if status.active {
                LogLevel::Info
            } else if status.requested.enabled {
                LogLevel::Warn
            } else {
                LogLevel::Info
            },
            "linux_rt_profile",
            json!({
                "profile": status.requested.profile_name(),
                "enabled": status.requested.enabled,
                "strict": status.requested.strict,
                "kernel_realtime": status.kernel_realtime,
                "memory_lock_applied": status.memory_lock_applied,
                "affinity_applied": status.affinity_applied_by_runtime,
                "scheduler": status.active_scheduler.map(|value| value.as_str()),
                "priority": status.active_priority,
                "cpu_affinity": status.active_cpu_affinity,
                "memory_locked_kb": status.memory_locked_kb,
                "warnings": status
                    .warnings
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>(),
                "errors": status
                    .errors
                    .iter()
                    .map(|value| value.as_str())
                    .collect::<Vec<_>>(),
            }),
        );
    }
    let control = handle.control();

    let settings = build_runtime_settings(
        bundle.as_ref(),
        ide_shell_mode,
        default_watchdog,
        default_fault,
        &SimulationPlan {
            enabled: simulation_enabled,
            time_scale: simulation_time_scale,
            warning: simulation_warning.clone(),
            controller: None,
        },
        selected_execution_backend,
        selected_execution_backend_source,
    );
    let auth_token_value = control_auth_token_value(bundle.as_ref());
    let web_config = resolve_web_config(bundle.as_ref(), &settings);
    let auth_token = Arc::new(Mutex::new(
        bundle
            .as_ref()
            .and_then(|bundle| bundle.runtime.control_auth_token.clone()),
    ));
    let pairing = bundle
        .as_ref()
        .map(|bundle| Arc::new(PairingStore::load(bundle.root.join("pairings.json"))));
    let historian = if let Some(bundle) = &bundle {
        if bundle.runtime.observability.enabled {
            let service = HistorianService::new(
                bundle.runtime.observability.clone(),
                Some(bundle.root.as_path()),
            )?;
            service.clone().start_sampler(debug.clone());
            Some(service)
        } else {
            None
        }
    } else {
        None
    };

    let (audit_tx, audit_rx) = std::sync::mpsc::channel();
    let audit_logger = logger.clone();
    std::thread::spawn(move || {
        for event in audit_rx {
            log_control_audit(&audit_logger, event);
        }
    });

    let hmi_descriptor = Arc::new(Mutex::new(HmiRuntimeDescriptor::from_sources(
        bundle.as_ref().map(|bundle| bundle.root.as_path()),
        &sources,
    )));
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
        realtime_status: realtime_status.clone(),
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
        hmi_descriptor,
        historian: historian.clone(),
        pairing: pairing.clone(),
    });
    spawn_hmi_descriptor_watcher(state.clone());

    let mut opcua_server: Option<OpcUaWireServer> = None;
    if let Some(bundle) = &bundle {
        let snapshot_control = control.clone();
        let snapshot_debug = debug.clone();
        let snapshot_provider = Arc::new(move || {
            let (tx, rx) = std::sync::mpsc::channel();
            if snapshot_control
                .send_command(ResourceCommand::Snapshot { respond_to: tx })
                .is_ok()
            {
                if let Ok(snapshot) = rx.recv_timeout(std::time::Duration::from_millis(250)) {
                    return Some(snapshot);
                }
            }
            snapshot_debug.snapshot()
        });
        opcua_server = start_wire_server(
            bundle.runtime.resource_name.as_str(),
            &bundle.runtime.opcua,
            snapshot_provider,
            Some(bundle.root.as_path()),
        )?;
    }

    let _server = ControlServer::start(control_endpoint.clone(), state.clone())?;
    let _mesh = if let Some(bundle) = &bundle {
        start_mesh(
            &bundle.runtime.mesh,
            bundle.runtime.resource_name.clone(),
            control.clone(),
        )?
    } else {
        None
    };
    let _discovery_handle = if let Some(bundle) = &bundle {
        if bundle.runtime.discovery.enabled {
            let web_listen = bundle.runtime.web.listen.as_str();
            let mesh_listen = _mesh
                .as_ref()
                .and_then(|service| service.discovery_mesh_listen());
            let handle = start_discovery(
                &bundle.runtime.discovery,
                &bundle.runtime.resource_name,
                &control_endpoint,
                Some(web_listen),
                bundle.runtime.web.tls,
                mesh_listen,
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
    let _web = if web_config.enabled {
        Some(start_web_server(
            &web_config,
            state.clone(),
            Some(discovery_state.clone()),
            pairing.clone(),
            bundle.as_ref().map(|bundle| bundle.root.clone()),
            tls_materials.clone(),
        )?)
    } else {
        None
    };

    start_gate.open();

    if show_banner {
        let web_url = web_config
            .enabled
            .then(|| format_web_url(web_config.listen.as_str(), web_config.tls));
        print_trust_banner(
            bundle.as_ref(),
            web_url.as_deref(),
            simulation_enabled,
            simulation_time_scale,
            startup_hmi_scaffold.as_ref(),
            selected_execution_backend,
            selected_execution_backend_source,
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
                opcua_server.as_ref().map(|server| server.endpoint_url()),
                simulation_enabled,
                simulation_time_scale,
                (
                    selected_execution_backend,
                    selected_execution_backend_source,
                ),
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
                "io_driver": bundle
                    .io
                    .drivers
                    .first()
                    .map(|driver| driver.name.to_string())
                    .unwrap_or_default(),
                "io_drivers": bundle
                    .io
                    .drivers
                    .iter()
                    .map(|driver| driver.name.to_string())
                    .collect::<Vec<_>>(),
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
                "web_tls": bundle.runtime.web.tls,
                "discovery_enabled": bundle.runtime.discovery.enabled,
                "mesh_enabled": bundle.runtime.mesh.enabled,
                "mesh_tls": bundle.runtime.mesh.tls,
                "runtime_cloud_profile": bundle.runtime.runtime_cloud_profile.as_str(),
                "runtime_cloud_wan_allow_write_rules": bundle.runtime.runtime_cloud_wan_allow_write.len(),
                "opcua_enabled": bundle.runtime.opcua.enabled,
                "opcua_endpoint": opcua_server
                    .as_ref()
                    .map(|server| server.endpoint_url().to_string()),
                "opcua_security_policy": bundle.runtime.opcua.security.policy.as_config_value(),
                "opcua_security_mode": bundle.runtime.opcua.security.mode.as_config_value(),
                "opcua_allow_anonymous": bundle.runtime.opcua.security.allow_anonymous,
                "opcua_username_set": bundle.runtime.opcua.username.is_some(),
                "opcua_exposed_patterns": bundle.runtime.opcua.expose.len(),
                "simulation_mode": if simulation_enabled { "simulation" } else { "production" },
                "simulation_time_scale": simulation_time_scale,
                "execution_backend": selected_execution_backend.as_str(),
                "execution_backend_source": selected_execution_backend_source.as_str(),
            }),
        );
    }

    let join_result = handle
        .join()
        .map_err(|_| anyhow::anyhow!("runtime thread panicked"));
    if let Some(server) = opcua_server.as_mut() {
        server.stop();
    }
    join_result?;
    logger.log(
        LogLevel::Debug,
        "runtime_exit",
        json!({ "status": "stopped" }),
    );
    Ok(())
}

fn parse_restart_mode(restart: &str) -> anyhow::Result<RestartMode> {
    match restart.to_ascii_lowercase().as_str() {
        "cold" => Ok(RestartMode::Cold),
        "warm" => Ok(RestartMode::Warm),
        _ => anyhow::bail!(
            "Invalid restart mode: {restart}. Expected: cold or warm. Tip: run trust-runtime play --help"
        ),
    }
}

fn resolve_execution_backend_selection(
    bundle: Option<&RuntimeBundle>,
    cli_override: Option<crate::cli::ExecutionBackendArg>,
) -> anyhow::Result<(
    trust_runtime::execution_backend::ExecutionBackend,
    trust_runtime::execution_backend::ExecutionBackendSource,
) > {
    if let Some(backend) = cli_override {
        let backend = match backend {
            crate::cli::ExecutionBackendArg::Vm => {
                trust_runtime::execution_backend::ExecutionBackend::BytecodeVm
            }
        };
        return Ok((
            backend,
            trust_runtime::execution_backend::ExecutionBackendSource::Flag,
        ));
    }
    if let Some(bundle) = bundle {
        return Ok((
            bundle.runtime.execution_backend,
            bundle.runtime.execution_backend_source,
        ));
    }
    Ok((
        trust_runtime::execution_backend::ExecutionBackend::BytecodeVm,
        trust_runtime::execution_backend::ExecutionBackendSource::Default,
    ))
}
