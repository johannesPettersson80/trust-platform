fn execute_case(
    case: &CaseDefinition,
    device: Option<&mut DeviceRun>,
) -> anyhow::Result<CaseArtifact> {
    match case.manifest.kind {
        CaseKind::Runtime => {
            let sources = load_case_sources(case)?;
            match device {
                None => execute_runtime_case(case, &sources),
                Some(device) => execute_runtime_case_device(case, &sources, device),
            }
        }
        CaseKind::CompileError => {
            let sources = load_case_sources(case)?;
            execute_compile_error_case(case, &sources)
        }
        CaseKind::ConnectorStatusTrace => execute_connector_status_trace_case(case),
    }
}

fn load_case_sources(case: &CaseDefinition) -> anyhow::Result<Vec<String>> {
    let mut sources = Vec::with_capacity(case.manifest.sources.len());
    let canonical_case_dir = case
        .dir
        .canonicalize()
        .with_context(|| format!("resolve case directory '{}'", case.dir.display()))?;
    for file in &case.manifest.sources {
        let relative = Path::new(file);
        if relative.is_absolute()
            || relative.components().any(|component| {
                matches!(
                    component,
                    std::path::Component::ParentDir
                        | std::path::Component::RootDir
                        | std::path::Component::Prefix(_)
                )
            })
        {
            bail!(
                "case source '{}' must remain inside '{}'",
                file,
                case.dir.display()
            );
        }
        let path = case.dir.join(relative);
        let resolved = path
            .canonicalize()
            .with_context(|| format!("resolve case source '{}'", path.display()))?;
        if !resolved.starts_with(&canonical_case_dir) {
            bail!(
                "case source '{}' must remain inside '{}'",
                file,
                case.dir.display()
            );
        }
        let text = fs::read_to_string(&path)
            .with_context(|| format!("read case source '{}'", path.display()))?;
        sources.push(text);
    }
    Ok(sources)
}

fn execute_compile_error_case(
    case: &CaseDefinition,
    sources: &[String],
) -> anyhow::Result<CaseArtifact> {
    // The front of the build reports the compile errors (specification 44 §3).
    let files = sources
        .iter()
        .map(|text| SourceFile::new(text.as_str()))
        .collect::<Vec<_>>();
    let compile_result = CompileSession::from_sources(files).declarations();
    let error = match compile_result {
        Ok(_) => bail!("compile_error case '{}' compiled successfully", case.id),
        Err(err) => err.to_string(),
    };
    Ok(CaseArtifact {
        payload: json!({
            "version": 1,
            "case_id": case.id,
            "category": case.category,
            "kind": "compile_error",
            "description": case.manifest.description,
            "compiled": false,
            "error": error
        }),
        cycles: None,
    })
}

fn execute_connector_status_trace_case(case: &CaseDefinition) -> anyhow::Result<CaseArtifact> {
    if case.manifest.connector_status_steps.is_empty() {
        bail!(
            "connector status trace case '{}' must declare connector_status_steps",
            case.id
        );
    }

    let mut trace = Vec::with_capacity(case.manifest.connector_status_steps.len());
    for (idx, step) in case.manifest.connector_status_steps.iter().enumerate() {
        let projection = project_connector_status_step(step)
            .with_context(|| format!("project connector status step {}", idx + 1))?;
        let state = connector_state_name(projection.state);
        let health = connector_health_name(projection.health);
        if let Some(expected) = step.expected_state.as_deref() {
            ensure_expected("state", state, expected)?;
        }
        if let Some(expected) = step.expected_health.as_deref() {
            ensure_expected("health", health, expected)?;
        }

        trace.push(json!({
            "step": idx + 1,
            "source": step.source,
            "source_state": step.state,
            "degraded_points": step.degraded_points,
            "state": state,
            "health": health,
            "detail": projection.detail,
        }));
    }

    Ok(CaseArtifact {
        payload: json!({
            "version": 1,
            "case_id": case.id,
            "category": case.category,
            "kind": "connector_status_trace",
            "description": case.manifest.description,
            "trace": trace
        }),
        cycles: Some(u64::try_from(case.manifest.connector_status_steps.len()).unwrap_or(u64::MAX)),
    })
}

fn project_connector_status_step(
    step: &ConnectorStatusTraceStep,
) -> anyhow::Result<trust_runtime::connectors::ConnectorStatusProjection> {
    match normalize_token(&step.source).as_str() {
        "ads_connection" => Ok(ads_connection_state_status(parse_ads_connection_state(
            &step.state,
        )?)),
        "ads_status" => Ok(ads_connection_status_state(
            parse_ads_connection_status_state(&step.state)?,
            step.degraded_points,
        )),
        "opcua_client" => Ok(opcua_client_status(
            parse_opcua_client_state(&step.state)?,
            step.degraded_points,
        )),
        "opcua_server" => Ok(opcua_server_snapshot_status(parse_opcua_server_state(
            &step.state,
        )?)),
        "mqtt_session" => Ok(mqtt_session_status(parse_mqtt_session_state(&step.state)?)),
        "modbus" => Ok(modbus_status(parse_modbus_state(&step.state)?)),
        "ethercat" => Ok(ethercat_status(parse_ethercat_state(&step.state)?)),
        "io_driver" => {
            let policy = step
                .error_policy
                .as_deref()
                .map(IoDriverErrorPolicy::parse)
                .transpose()
                .context("parse io_driver error_policy")?
                .unwrap_or(IoDriverErrorPolicy::Fault);
            Ok(io_driver_status(&parse_io_driver_health(step)?, policy))
        }
        other => bail!("unsupported connector status source '{other}'"),
    }
}

fn parse_ads_connection_state(state: &str) -> anyhow::Result<AdsConnectionState> {
    Ok(match normalize_token(state).as_str() {
        "disconnected" => AdsConnectionState::Disconnected,
        "connecting" => AdsConnectionState::Connecting,
        "connected" => AdsConnectionState::Connected,
        "reconnecting" => AdsConnectionState::Reconnecting,
        "faulted" => AdsConnectionState::Faulted,
        other => bail!("unsupported ADS connection state '{other}'"),
    })
}

fn parse_ads_connection_status_state(state: &str) -> anyhow::Result<AdsConnectionStatusState> {
    Ok(match normalize_token(state).as_str() {
        "connected" => AdsConnectionStatusState::Connected,
        "reconnecting" => AdsConnectionStatusState::Reconnecting,
        "not_ready" => AdsConnectionStatusState::NotReady,
        "faulted" => AdsConnectionStatusState::Faulted,
        "stale" => AdsConnectionStatusState::Stale,
        "disabled" => AdsConnectionStatusState::Disabled,
        "unknown" => AdsConnectionStatusState::Unknown,
        other => bail!("unsupported ADS status state '{other}'"),
    })
}

fn parse_opcua_client_state(state: &str) -> anyhow::Result<OpcUaClientConnectionState> {
    Ok(match normalize_token(state).as_str() {
        "disabled" => OpcUaClientConnectionState::Disabled,
        "configured" => OpcUaClientConnectionState::Configured,
        "connecting" => OpcUaClientConnectionState::Connecting,
        "connected" => OpcUaClientConnectionState::Connected,
        "reconnecting" => OpcUaClientConnectionState::Reconnecting,
        "stale" => OpcUaClientConnectionState::Stale,
        "faulted" => OpcUaClientConnectionState::Faulted,
        other => bail!("unsupported OPC UA client state '{other}'"),
    })
}

fn parse_opcua_server_state(state: &str) -> anyhow::Result<OpcUaServerSnapshotState> {
    Ok(match normalize_token(state).as_str() {
        "disabled" => OpcUaServerSnapshotState::Disabled,
        "starting" => OpcUaServerSnapshotState::Starting,
        "no_snapshot" | "not_ready" => OpcUaServerSnapshotState::NoSnapshot,
        "snapshot_ready" | "ready" => OpcUaServerSnapshotState::SnapshotReady,
        "faulted" => OpcUaServerSnapshotState::Faulted,
        other => bail!("unsupported OPC UA server state '{other}'"),
    })
}

fn parse_mqtt_session_state(state: &str) -> anyhow::Result<MqttSessionProjection> {
    Ok(match normalize_token(state).as_str() {
        "disabled" => MqttSessionProjection::Disabled,
        "disconnected" => MqttSessionProjection::Disconnected,
        "connecting" => MqttSessionProjection::Connecting,
        "connected_fresh" | "fresh" => MqttSessionProjection::ConnectedFresh,
        "connected_stale" | "stale" => MqttSessionProjection::ConnectedStale,
        "faulted" => MqttSessionProjection::Faulted,
        other => bail!("unsupported MQTT session state '{other}'"),
    })
}

fn parse_modbus_state(state: &str) -> anyhow::Result<ModbusProjection> {
    Ok(match normalize_token(state).as_str() {
        "disabled" => ModbusProjection::Disabled,
        "ready" => ModbusProjection::Ready,
        "timeout" => ModbusProjection::Timeout,
        "protocol_error" => ModbusProjection::ProtocolError,
        "faulted" => ModbusProjection::Faulted,
        other => bail!("unsupported Modbus state '{other}'"),
    })
}

fn parse_ethercat_state(state: &str) -> anyhow::Result<EthercatProjection> {
    Ok(match normalize_token(state).as_str() {
        "disabled" => EthercatProjection::Disabled,
        "operational" | "ready" => EthercatProjection::Operational,
        "degraded" => EthercatProjection::Degraded,
        "reconnecting" => EthercatProjection::Reconnecting,
        "faulted" => EthercatProjection::Faulted,
        other => bail!("unsupported EtherCAT state '{other}'"),
    })
}

fn parse_io_driver_health(step: &ConnectorStatusTraceStep) -> anyhow::Result<IoDriverHealth> {
    let detail = step
        .detail
        .clone()
        .unwrap_or_else(|| "simulated conformance status".to_string())
        .into();
    Ok(match normalize_token(&step.state).as_str() {
        "ok" | "ready" => IoDriverHealth::Ok,
        "degraded" => IoDriverHealth::Degraded { error: detail },
        "faulted" => IoDriverHealth::Faulted { error: detail },
        other => bail!("unsupported IO driver health '{other}'"),
    })
}

fn ensure_expected(field: &str, actual: &str, expected: &str) -> anyhow::Result<()> {
    if actual == normalize_token(expected) {
        Ok(())
    } else {
        bail!("expected connector {field} '{expected}', got '{actual}'")
    }
}

fn connector_state_name(state: ConnectorState) -> &'static str {
    match state {
        ConnectorState::Disabled => "disabled",
        ConnectorState::Configured => "configured",
        ConnectorState::Starting => "starting",
        ConnectorState::Ready => "ready",
        ConnectorState::Degraded => "degraded",
        ConnectorState::Reconnecting => "reconnecting",
        ConnectorState::Stale => "stale",
        ConnectorState::NotReady => "not_ready",
        ConnectorState::Faulted => "faulted",
    }
}

fn connector_health_name(health: ConnectorHealth) -> &'static str {
    match health {
        ConnectorHealth::Ok => "ok",
        ConnectorHealth::Degraded => "degraded",
        ConnectorHealth::Faulted => "faulted",
        ConnectorHealth::Unknown => "unknown",
    }
}

fn normalize_token(value: &str) -> String {
    value.trim().to_ascii_lowercase().replace('-', "_")
}
