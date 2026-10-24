// Executable installation precedes service configuration in load_runtime.
// This operation never reloads code or executes initialization a second time.
fn apply_bundle_runtime_services(
    runtime: &mut Runtime,
    bundle: &RuntimeBundle,
) -> anyhow::Result<()> {
    if bundle.runtime.bundle_version != 1 {
        anyhow::bail!(
            "unsupported bundle version {}",
            bundle.runtime.bundle_version
        );
    }

    runtime.set_watchdog_policy(bundle.runtime.watchdog);
    runtime.set_fault_policy(bundle.runtime.fault_policy);
    runtime.set_io_safe_state(bundle.io.safe_state.clone());
    runtime.configure_openot_telemetry(&bundle.runtime.openot, Some(&bundle.root))?;

    let registry = IoDriverRegistry::default_registry();
    let mut resolved_drivers = Vec::with_capacity(bundle.io.drivers.len());
    for driver in &bundle.io.drivers {
        let mut resolved_driver = driver.clone();
        if !driver.enabled {
            resolved_drivers.push(resolved_driver);
            continue;
        }
        if driver.name.eq_ignore_ascii_case("mqtt") || driver.name.eq_ignore_ascii_case("mqtt-tcp")
        {
            resolved_driver.params =
                trust_runtime::io::resolve_mqtt_tag_mappings(runtime, &resolved_driver.params)?;
        }
        if let Some(spec) = registry
            .build(resolved_driver.name.as_str(), &resolved_driver.params)
            .map_err(anyhow::Error::from)?
        {
            runtime.add_io_driver(spec.name, spec.driver);
        }
        resolved_drivers.push(resolved_driver);
    }

    match bundle.runtime.retain_mode {
        trust_runtime::watchdog::RetainMode::File => {
            let store = bundle.runtime.retain_path.as_ref().map(|path| {
                let path = if path.is_relative() {
                    bundle.root.join(path)
                } else {
                    path.clone()
                };
                Box::new(FileRetainStore::new(path)) as _
            });
            runtime.set_retain_store(store, Some(bundle.runtime.retain_save_interval));
        }
        trust_runtime::watchdog::RetainMode::None => {
            runtime.set_retain_store(None, None);
        }
    }

    runtime.annotate_io_binding_sources(&resolved_drivers);
    start_ads_runtime(runtime, bundle)?;
    start_opcua_client_runtime(runtime, bundle)?;

    Ok(())
}

/// [`apply_bundle_runtime_services`] for a compiled program (specification 44 §8.8): the watchdog
/// and fault policies, the safe outputs, the `io.toml` drivers and the retain file. The services
/// that reach the program's variables by name, the ADS and OPC UA clients, the MQTT tag mappings and
/// the OpenOT telemetry, are refused for a compiled program until they read its symbol table
/// (specification 44 §13.3).
fn apply_bundle_compiled_services(
    program: &mut trust_runtime::CompiledRuntime,
    bundle: &RuntimeBundle,
) -> anyhow::Result<()> {
    if bundle.runtime.bundle_version != 1 {
        anyhow::bail!(
            "unsupported bundle version {}",
            bundle.runtime.bundle_version
        );
    }
    let unsupported = |setting: &str| {
        anyhow::anyhow!(
            "{setting}: a compiled program's variables are not yet reached by this service"
        )
    };
    if bundle.runtime.ads.enabled {
        return Err(unsupported("runtime.ads.enabled"));
    }
    if bundle.runtime.opcua_client.enabled {
        return Err(unsupported("runtime.opcua_client.enabled"));
    }
    if bundle.runtime.openot.enabled {
        return Err(unsupported("runtime.openot.enabled"));
    }

    program.set_watchdog_policy(bundle.runtime.watchdog);
    program.set_fault_policy(bundle.runtime.fault_policy);
    program.set_io_safe_state(bundle.io.safe_state.clone());

    let registry = IoDriverRegistry::default_registry();
    for driver in bundle.io.drivers.iter().filter(|driver| driver.enabled) {
        if (driver.name.eq_ignore_ascii_case("mqtt")
            || driver.name.eq_ignore_ascii_case("mqtt-tcp"))
            && trust_runtime::io::has_mqtt_tag_mappings(&driver.params)?
        {
            return Err(unsupported("io.params.mappings of the mqtt driver"));
        }
        if let Some(spec) = registry
            .build(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?
        {
            program.add_io_driver(spec.name, spec.driver);
        }
    }

    match bundle.runtime.retain_mode {
        trust_runtime::watchdog::RetainMode::File => {
            let store = bundle.runtime.retain_path.as_ref().map(|path| {
                let path = if path.is_relative() {
                    bundle.root.join(path)
                } else {
                    path.clone()
                };
                Box::new(FileRetainStore::new(path)) as _
            });
            program.set_retain_store(store, Some(bundle.runtime.retain_save_interval))?;
        }
        trust_runtime::watchdog::RetainMode::None => {
            program.set_retain_store(None, None)?;
        }
    }
    Ok(())
}

fn start_openot_persistence_service(
    bundle: &RuntimeBundle,
) -> anyhow::Result<Option<OpenOtPersistenceService>> {
    OpenOtPersistenceService::start(&bundle.runtime.openot, &bundle.root)
        .map_err(|error| anyhow::anyhow!("start OpenOT persistence: {error:?}"))
}

#[cfg(feature = "ads-wire")]
fn start_ads_runtime(runtime: &mut Runtime, bundle: &RuntimeBundle) -> anyhow::Result<()> {
    start_ads_runtime_with_factory(runtime, bundle, |connection| {
        Ok(trust_runtime::ads::HostAdsClient::new(
            connection.route.clone(),
        ))
    })
}

#[cfg(not(feature = "ads-wire"))]
fn start_ads_runtime(_runtime: &mut Runtime, bundle: &RuntimeBundle) -> anyhow::Result<()> {
    if bundle.runtime.ads.enabled {
        anyhow::bail!(
            "runtime.ads.enabled=true requires trust-runtime built with feature 'ads-wire'"
        );
    }
    Ok(())
}

#[cfg(feature = "opcua-wire")]
fn start_opcua_client_runtime(runtime: &mut Runtime, bundle: &RuntimeBundle) -> anyhow::Result<()> {
    start_opcua_client_runtime_with_factory(runtime, bundle, |_connection| {
        Ok(trust_runtime::opcua::OpcUaWireClientTransport::new())
    })
}

#[cfg(feature = "opcua-wire")]
fn start_opcua_client_runtime_with_factory<T, F>(
    runtime: &mut Runtime,
    bundle: &RuntimeBundle,
    mut transport_factory: F,
) -> anyhow::Result<()>
where
    T: trust_runtime::opcua::OpcUaClientTransport + Send + 'static,
    F: FnMut(&trust_runtime::opcua::OpcUaClientConnectionConfig) -> anyhow::Result<T>,
{
    if !bundle.runtime.opcua_client.enabled {
        return Ok(());
    }
    let Some(config) = bundle.opcua_client.as_ref() else {
        anyhow::bail!("runtime.opcua_client.enabled=true but no OPC UA client config was loaded");
    };
    runtime.reset_opcua_client_connections()?;
    runtime.set_opcua_client_deployed_config_hash(bundle.opcua_client_config_hash.clone());
    let worker_tick_interval = std::time::Duration::from_millis(
        u64::try_from(bundle.runtime.opcua_client.poll_interval.as_millis()).unwrap_or(20),
    );
    for connection in &config.connections {
        let transport = transport_factory(connection)?;
        runtime
            .start_opcua_client_connection(connection, transport, worker_tick_interval)
            .map_err(anyhow::Error::from)?;
    }
    Ok(())
}

#[cfg(not(feature = "opcua-wire"))]
fn start_opcua_client_runtime(
    _runtime: &mut Runtime,
    bundle: &RuntimeBundle,
) -> anyhow::Result<()> {
    if bundle.runtime.opcua_client.enabled {
        anyhow::bail!(
            "runtime.opcua_client.enabled=true requires trust-runtime built with feature 'opcua-wire'"
        );
    }
    Ok(())
}

#[cfg(any(test, feature = "ads-wire"))]
fn start_ads_runtime_with_factory<T, F>(
    runtime: &mut Runtime,
    bundle: &RuntimeBundle,
    mut transport_factory: F,
) -> anyhow::Result<()>
where
    T: trust_runtime::ads::AdsTransport + Send + 'static,
    F: FnMut(&trust_runtime::ads::AdsConnectionConfig) -> anyhow::Result<T>,
{
    if !bundle.runtime.ads.enabled {
        return Ok(());
    }
    let Some(config) = bundle.ads.as_ref() else {
        anyhow::bail!("runtime.ads.enabled=true but no ADS client config was loaded");
    };
    runtime.set_ads_deployed_config_hash(bundle.ads_config_hash.clone());
    let worker_tick_interval = std::time::Duration::from_millis(
        u64::try_from(bundle.runtime.ads.worker_tick_interval.as_millis()).unwrap_or(20),
    );
    for connection in &config.connections {
        let transport = transport_factory(connection)?;
        runtime
            .start_ads_connection(connection, transport, worker_tick_interval)
            .map_err(anyhow::Error::from)?;
    }
    Ok(())
}

fn parse_control_endpoint(bundle: Option<&RuntimeBundle>) -> anyhow::Result<ControlEndpoint> {
    if let Some(bundle) = bundle {
        Ok(ControlEndpoint::parse(
            bundle.runtime.control_endpoint.as_str(),
        )?)
    } else {
        Ok(ControlEndpoint::parse("tcp://127.0.0.1:9000")?)
    }
}

fn ensure_control_auth_requirements(
    control_endpoint: &ControlEndpoint,
    bundle: Option<&RuntimeBundle>,
    ide_shell_mode: bool,
) -> anyhow::Result<()> {
    if matches!(control_endpoint, ControlEndpoint::Tcp(_)) {
        let token = bundle.and_then(|bundle| bundle.runtime.control_auth_token.as_ref());
        if token.is_none() && !ide_shell_mode {
            anyhow::bail!("tcp control endpoint requires runtime.control.auth_token");
        }
    }
    Ok(())
}
