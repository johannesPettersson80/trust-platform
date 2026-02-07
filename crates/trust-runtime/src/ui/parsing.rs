use super::*;

pub(super) fn parse_status(response: &serde_json::Value) -> Option<StatusSnapshot> {
    let result = response.get("result")?;
    Some(StatusSnapshot {
        state: result.get("state")?.as_str()?.to_string(),
        fault: result
            .get("fault")
            .and_then(|v| v.as_str())
            .unwrap_or("none")
            .to_string(),
        resource: result
            .get("resource")
            .and_then(|v| v.as_str())
            .unwrap_or("resource")
            .to_string(),
        uptime_ms: result
            .get("uptime_ms")
            .and_then(|v| v.as_u64())
            .unwrap_or_default(),
        cycle_min: result
            .get("metrics")
            .and_then(|m| m.get("cycle_ms"))
            .and_then(|m| m.get("min"))
            .and_then(|v| v.as_f64())
            .unwrap_or_default(),
        cycle_avg: result
            .get("metrics")
            .and_then(|m| m.get("cycle_ms"))
            .and_then(|m| m.get("avg"))
            .and_then(|v| v.as_f64())
            .unwrap_or_default(),
        cycle_max: result
            .get("metrics")
            .and_then(|m| m.get("cycle_ms"))
            .and_then(|m| m.get("max"))
            .and_then(|v| v.as_f64())
            .unwrap_or_default(),
        cycle_last: result
            .get("metrics")
            .and_then(|m| m.get("cycle_ms"))
            .and_then(|m| m.get("last"))
            .and_then(|v| v.as_f64())
            .unwrap_or_default(),
        overruns: result
            .get("metrics")
            .and_then(|m| m.get("overruns"))
            .and_then(|v| v.as_u64())
            .unwrap_or_default(),
        faults: result
            .get("metrics")
            .and_then(|m| m.get("faults"))
            .and_then(|v| v.as_u64())
            .unwrap_or_default(),
        drivers: result
            .get("drivers")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .map(|entry| DriverSnapshot {
                        name: entry
                            .get("name")
                            .and_then(|v| v.as_str())
                            .unwrap_or("driver")
                            .to_string(),
                        status: entry
                            .get("status")
                            .and_then(|v| v.as_str())
                            .unwrap_or("unknown")
                            .to_string(),
                        error: entry
                            .get("error")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                    })
                    .collect()
            })
            .unwrap_or_default(),
        debug_enabled: result
            .get("debug_enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or_default(),
        control_mode: result
            .get("control_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("unknown")
            .to_string(),
        simulation_mode: result
            .get("simulation_mode")
            .and_then(|v| v.as_str())
            .unwrap_or("production")
            .to_string(),
        simulation_time_scale: result
            .get("simulation_time_scale")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(1),
        simulation_warning: result
            .get("simulation_warning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}

pub(super) fn parse_tasks(response: &serde_json::Value) -> Vec<TaskSnapshot> {
    response
        .get("result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|task| TaskSnapshot {
                    name: task
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("task")
                        .to_string(),
                    last_ms: task
                        .get("last_ms")
                        .and_then(|v| v.as_f64())
                        .unwrap_or_default(),
                    avg_ms: task
                        .get("avg_ms")
                        .and_then(|v| v.as_f64())
                        .unwrap_or_default(),
                    max_ms: task
                        .get("max_ms")
                        .and_then(|v| v.as_f64())
                        .unwrap_or_default(),
                    overruns: task
                        .get("overruns")
                        .and_then(|v| v.as_u64())
                        .unwrap_or_default(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_io(response: &serde_json::Value) -> Vec<IoEntry> {
    response
        .get("result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|entry| IoEntry {
                    name: entry
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("signal")
                        .to_string(),
                    address: entry
                        .get("address")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-")
                        .to_string(),
                    value: entry
                        .get("value")
                        .map(|value| {
                            if let Some(text) = value.as_str() {
                                text.to_string()
                            } else {
                                value.to_string()
                            }
                        })
                        .unwrap_or_else(|| "-".to_string()),
                    direction: entry
                        .get("direction")
                        .and_then(|v| v.as_str())
                        .unwrap_or("-")
                        .to_string(),
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_events(response: &serde_json::Value) -> Vec<EventSnapshot> {
    response
        .get("result")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .map(|entry| {
                    let label = entry
                        .get("code")
                        .and_then(|v| v.as_str())
                        .unwrap_or("EVT")
                        .to_string();
                    let kind = match entry
                        .get("level")
                        .and_then(|v| v.as_str())
                        .unwrap_or("info")
                        .to_ascii_lowercase()
                        .as_str()
                    {
                        "fault" | "error" => EventKind::Fault,
                        "warn" | "warning" => EventKind::Warn,
                        _ => EventKind::Info,
                    };
                    EventSnapshot {
                        label,
                        kind,
                        timestamp: entry
                            .get("timestamp")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        message: entry
                            .get("message")
                            .and_then(|v| v.as_str())
                            .unwrap_or_default()
                            .to_string(),
                    }
                })
                .collect()
        })
        .unwrap_or_default()
}

pub(super) fn parse_settings(response: &serde_json::Value) -> Option<SettingsSnapshot> {
    let result = response.get("result")?;
    Some(SettingsSnapshot {
        log_level: result
            .get("log.level")
            .and_then(|v| v.as_str())
            .unwrap_or("info")
            .to_string(),
        watchdog_enabled: result
            .get("watchdog.enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        watchdog_timeout_ms: result
            .get("watchdog.timeout_ms")
            .and_then(|v| v.as_i64())
            .unwrap_or(0),
        watchdog_action: result
            .get("watchdog.action")
            .and_then(|v| v.as_str())
            .unwrap_or("none")
            .to_string(),
        fault_policy: result
            .get("fault.policy")
            .and_then(|v| v.as_str())
            .unwrap_or("warn")
            .to_string(),
        retain_mode: result
            .get("retain.mode")
            .and_then(|v| v.as_str())
            .unwrap_or("off")
            .to_string(),
        retain_save_interval_ms: result
            .get("retain.save_interval_ms")
            .and_then(|v| v.as_i64()),
        web_listen: result
            .get("web.listen")
            .and_then(|v| v.as_str())
            .unwrap_or("127.0.0.1:8080")
            .to_string(),
        web_auth: result
            .get("web.auth")
            .and_then(|v| v.as_str())
            .unwrap_or("off")
            .to_string(),
        discovery_enabled: result
            .get("discovery.enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        mesh_enabled: result
            .get("mesh.enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        mesh_publish: result
            .get("mesh.publish")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|entry| entry.as_str().map(|s| s.to_string()))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        mesh_subscribe: result
            .get("mesh.subscribe")
            .and_then(|v| v.as_array())
            .map(|arr| {
                arr.iter()
                    .filter_map(|entry| {
                        let topic = entry.get("topic")?.as_str()?.to_string();
                        let alias = entry.get("alias")?.as_str()?.to_string();
                        Some((topic, alias))
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default(),
        control_mode: result
            .get("control.mode")
            .and_then(|v| v.as_str())
            .unwrap_or("ro")
            .to_string(),
        simulation_enabled: result
            .get("simulation.enabled")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        simulation_time_scale: result
            .get("simulation.time_scale")
            .and_then(|v| v.as_u64())
            .and_then(|v| u32::try_from(v).ok())
            .unwrap_or(1),
        simulation_mode: result
            .get("simulation.mode")
            .and_then(|v| v.as_str())
            .unwrap_or("production")
            .to_string(),
        simulation_warning: result
            .get("simulation.warning")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_string(),
    })
}
