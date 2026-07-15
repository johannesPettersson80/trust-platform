fn apply_bundle_runtime_overrides(
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
    for driver in &bundle.io.drivers {
        if !driver.enabled {
            continue;
        }
        if let Some(spec) = registry
            .build(driver.name.as_str(), &driver.params)
            .map_err(anyhow::Error::from)?
        {
            runtime.add_io_driver(spec.name, spec.driver);
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
            runtime.set_retain_store(store, Some(bundle.runtime.retain_save_interval));
        }
        trust_runtime::watchdog::RetainMode::None => {
            runtime.set_retain_store(None, None);
        }
    }

    if let Err(err) =
        runtime.apply_bytecode_bytes(&bundle.bytecode, Some(&bundle.runtime.resource_name))
    {
        anyhow::bail!(
            "failed to apply bytecode metadata: {err} (project folder may require sources)"
        );
    }
    trust_runtime::io::annotate_io_binding_sources(runtime.io_mut(), &bundle.io.drivers);
    start_ads_runtime(runtime, bundle)?;
    start_opcua_client_runtime(runtime, bundle)?;

    Ok(())
}

#[cfg(feature = "ads-wire")]
fn start_ads_runtime(runtime: &mut Runtime, bundle: &RuntimeBundle) -> anyhow::Result<()> {
    start_ads_runtime_with_factory(runtime, bundle, |connection| {
        Ok(trust_runtime::ads::AdsRsTransport::new(
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
