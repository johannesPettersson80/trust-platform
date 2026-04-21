pub(super) fn handle_config_get(id: u64, state: &ControlState) -> ControlResponse {
    let settings = match state.settings.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "settings unavailable".into()),
    };
    let auth = state.auth_token.lock().ok();
    let auth_set = auth
        .as_ref()
        .and_then(|value| value.as_ref())
        .map(|value| value.len())
        .unwrap_or(0);
    let observability = state.historian.as_ref().map(|hist| hist.config().clone());
    let observability_alerts = observability
        .as_ref()
        .map(|cfg| {
            cfg.alerts
                .iter()
                .map(|rule| {
                    let mut item = serde_json::Map::new();
                    item.insert(
                        "name".to_string(),
                        serde_json::Value::String(rule.name.to_string()),
                    );
                    item.insert(
                        "variable".to_string(),
                        serde_json::Value::String(rule.variable.to_string()),
                    );
                    item.insert(
                        "above".to_string(),
                        rule.above
                            .and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    item.insert(
                        "below".to_string(),
                        rule.below
                            .and_then(serde_json::Number::from_f64)
                            .map(serde_json::Value::Number)
                            .unwrap_or(serde_json::Value::Null),
                    );
                    item.insert(
                        "debounce_samples".to_string(),
                        serde_json::Value::Number(serde_json::Number::from(rule.debounce_samples)),
                    );
                    item.insert(
                        "hook".to_string(),
                        rule.hook
                            .as_ref()
                            .map(|value| serde_json::Value::String(value.to_string()))
                            .unwrap_or(serde_json::Value::Null),
                    );
                    serde_json::Value::Object(item)
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    ControlResponse::ok(
        id,
        json!({
            "log.level": settings.log_level.as_str(),
            "watchdog.enabled": settings.watchdog.enabled,
            "watchdog.timeout_ms": settings.watchdog.timeout.as_millis(),
            "watchdog.action": format!("{:?}", settings.watchdog.action),
            "resource.cycle_interval_ms": settings.cycle_interval.as_millis(),
            "runtime.execution_backend": settings.execution_backend.as_str(),
            "runtime.execution_backend_source": settings.execution_backend_source.as_str(),
            "fault.policy": format!("{:?}", settings.fault_policy),
            "retain.mode": format!("{:?}", settings.retain_mode),
            "retain.save_interval_ms": settings.retain_save_interval.map(|val| val.as_millis()),
            "web.enabled": settings.web.enabled,
            "web.listen": settings.web.listen.as_str(),
            "web.auth": settings.web.auth.as_str(),
            "web.tls": settings.web.tls,
            "discovery.enabled": settings.discovery.enabled,
            "discovery.service_name": settings.discovery.service_name.as_str(),
            "discovery.advertise": settings.discovery.advertise,
            "discovery.interfaces": settings.discovery.interfaces.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "mesh.enabled": settings.mesh.enabled,
            "mesh.role": settings.mesh.role.as_str(),
            "mesh.listen": settings.mesh.listen.as_str(),
            "mesh.connect": settings.mesh.connect.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "mesh.tls": settings.mesh.tls,
            "mesh.auth_token_set": settings.mesh.auth_token.as_ref().map(|t| t.len()).unwrap_or(0) > 0,
            "mesh.publish": settings.mesh.publish.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "mesh.subscribe": settings
                .mesh
                .subscribe
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        serde_json::Value::String(v.as_str().to_string()),
                    )
                })
                .collect::<serde_json::Map<_, _>>(),
            "mesh.zenohd_version": settings.mesh.zenohd_version.as_str(),
            "mesh.plugin_versions": settings
                .mesh
                .plugin_versions
                .iter()
                .map(|(k, v)| {
                    (
                        k.as_str().to_string(),
                        serde_json::Value::String(v.as_str().to_string()),
                    )
                })
                .collect::<serde_json::Map<_, _>>(),
            "runtime_cloud.profile": settings.runtime_cloud.profile.as_str(),
            "runtime_cloud.wan.allow_write": settings
                .runtime_cloud
                .wan_allow_write
                .iter()
                .map(|rule| {
                    json!({
                        "action": rule.action.as_str(),
                        "target": rule.target.as_str(),
                    })
                })
                .collect::<Vec<_>>(),
            "runtime_cloud.links.transports": settings
                .runtime_cloud
                .link_preferences
                .iter()
                .map(|rule| {
                    json!({
                        "source": rule.source.as_str(),
                        "target": rule.target.as_str(),
                        "transport": rule.transport.as_str(),
                    })
                })
                .collect::<Vec<_>>(),
            "realtime.profile": settings.realtime.profile_name(),
            "realtime.enabled": settings.realtime.enabled,
            "realtime.require_preempt_rt_kernel": settings.realtime.require_preempt_rt_kernel,
            "realtime.lock_memory": settings.realtime.lock_memory,
            "realtime.scheduler": settings.realtime.scheduler.as_str(),
            "realtime.priority": settings.realtime.priority,
            "realtime.cpu_affinity": settings.realtime.cpu_affinity,
            "realtime.strict": settings.realtime.strict,
            "opcua.enabled": settings.opcua.enabled,
            "opcua.listen": settings.opcua.listen.as_str(),
            "opcua.endpoint_path": settings.opcua.endpoint_path.as_str(),
            "opcua.namespace_uri": settings.opcua.namespace_uri.as_str(),
            "opcua.publish_interval_ms": settings.opcua.publish_interval_ms,
            "opcua.max_nodes": settings.opcua.max_nodes,
            "opcua.expose": settings.opcua.expose.iter().map(|v| v.as_str()).collect::<Vec<_>>(),
            "opcua.security_policy": settings.opcua.security_policy.as_str(),
            "opcua.security_mode": settings.opcua.security_mode.as_str(),
            "opcua.allow_anonymous": settings.opcua.allow_anonymous,
            "opcua.username_set": settings.opcua.username_set,
            "control.auth_token_set": auth_set > 0,
            "control.auth_token_length": if auth_set > 0 { Some(auth_set) } else { None },
            "control.debug_enabled": state.debug_enabled.load(Ordering::Relaxed),
            "control.mode": state
                .control_mode
                .lock()
                .map(|mode| format!("{:?}", *mode))
                .unwrap_or_else(|_| "Production".to_string()),
            "simulation.enabled": settings.simulation.enabled,
            "simulation.time_scale": settings.simulation.time_scale,
            "simulation.mode": settings.simulation.mode_label.as_str(),
            "simulation.warning": settings.simulation.warning.as_str(),
            "observability.enabled": observability.as_ref().map(|cfg| cfg.enabled).unwrap_or(false),
            "observability.sample_interval_ms": observability.as_ref().map(|cfg| cfg.sample_interval_ms),
            "observability.mode": observability.as_ref().map(|cfg| match cfg.mode {
                crate::historian::RecordingMode::All => "all",
                crate::historian::RecordingMode::Allowlist => "allowlist",
            }),
            "observability.include": observability
                .as_ref()
                .map(|cfg| cfg.include.iter().map(|entry| entry.as_str()).collect::<Vec<_>>())
                .unwrap_or_default(),
            "observability.history_path": observability.as_ref().map(|cfg| cfg.history_path.display().to_string()),
            "observability.max_entries": observability.as_ref().map(|cfg| cfg.max_entries),
            "observability.prometheus_enabled": observability.as_ref().map(|cfg| cfg.prometheus_enabled),
            "observability.prometheus_path": observability.as_ref().map(|cfg| cfg.prometheus_path.to_string()),
            "observability.alerts": observability_alerts,
            "hmi.read_only": true,
        }),
    )
}
