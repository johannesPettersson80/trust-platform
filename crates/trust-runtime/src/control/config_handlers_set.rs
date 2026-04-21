pub(super) fn handle_config_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    macro_rules! parse_or_error {
        ($expr:expr) => {
            match $expr {
                Ok(value) => value,
                Err(error) => return ControlResponse::error(id, error),
            }
        };
    }

    let params = match params {
        Some(params) => params,
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let params = match params.as_object() {
        Some(params) => params,
        None => {
            return ControlResponse::error(
                id,
                "invalid config payload: params must be an object".into(),
            )
        }
    };
    let mut settings_guard = match state.settings.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "settings unavailable".into()),
    };
    let mut settings = settings_guard.clone();
    let mut updated = Vec::new();
    let mut restart_required = Vec::new();
    let mut auth_token = match state.auth_token.lock() {
        Ok(guard) => guard.clone(),
        Err(_) => return ControlResponse::error(id, "auth token unavailable".into()),
    };
    let mut auth_changed = false;
    if let Some(value) = params.get("control.auth_token") {
        if value.is_null() {
            if state.control_requires_auth {
                return ControlResponse::error(id, "auth token required for tcp endpoints".into());
            }
            auth_token = None;
            auth_changed = true;
            updated.push("control.auth_token");
        } else if let Some(token) = value.as_str() {
            let token = token.trim();
            if token.is_empty() {
                return ControlResponse::error(
                    id,
                    config_value_error("control.auth_token", "must not be empty"),
                );
            }
            auth_token = Some(SmolStr::new(token));
            auth_changed = true;
            updated.push("control.auth_token");
        } else {
            return ControlResponse::error(
                id,
                config_type_error("control.auth_token", "string or null"),
            );
        }
    }

    let mut control_mode = match state.control_mode.lock() {
        Ok(guard) => *guard,
        Err(_) => return ControlResponse::error(id, "control mode unavailable".into()),
    };
    let mut control_mode_changed = false;
    let mut debug_enabled = state.debug_enabled.load(Ordering::Relaxed);
    let mut debug_enabled_changed = false;

    for (key, value) in params {
        match key.as_str() {
            "control.auth_token" => {}
            "log.level" => {
                let level = parse_or_error!(expect_non_empty_string(key, value));
                settings.log_level = SmolStr::new(level);
                updated.push("log.level");
            }
            "watchdog.enabled" => {
                settings.watchdog.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("watchdog.enabled");
            }
            "watchdog.timeout_ms" => {
                let timeout = parse_or_error!(expect_positive_i64(key, value));
                settings.watchdog.timeout = crate::value::Duration::from_millis(timeout);
                updated.push("watchdog.timeout_ms");
            }
            "watchdog.action" => {
                let action = parse_or_error!(expect_non_empty_string(key, value));
                settings.watchdog.action =
                    parse_or_error!(crate::watchdog::WatchdogAction::parse(action)
                        .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("watchdog.action");
            }
            "fault.policy" => {
                let policy = parse_or_error!(expect_non_empty_string(key, value));
                settings.fault_policy =
                    parse_or_error!(crate::watchdog::FaultPolicy::parse(policy)
                        .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("fault.policy");
            }
            "retain.save_interval_ms" => {
                let interval = parse_or_error!(expect_positive_i64(key, value));
                settings.retain_save_interval = Some(crate::value::Duration::from_millis(interval));
                updated.push("retain.save_interval_ms");
            }
            "retain.mode" => {
                let mode = parse_or_error!(expect_non_empty_string(key, value));
                settings.retain_mode = parse_or_error!(crate::watchdog::RetainMode::parse(mode)
                    .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("retain.mode");
                restart_required.push("retain.mode");
            }
            "web.enabled" => {
                settings.web.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("web.enabled");
                restart_required.push("web.enabled");
            }
            "web.listen" => {
                let listen = parse_or_error!(expect_non_empty_string(key, value));
                settings.web.listen = SmolStr::new(listen);
                updated.push("web.listen");
                restart_required.push("web.listen");
            }
            "web.auth" => {
                let auth = parse_or_error!(expect_non_empty_string(key, value));
                if auth.eq_ignore_ascii_case("token") && auth_token.is_none() {
                    return ControlResponse::error(
                        id,
                        config_value_error("web.auth", "token mode requires control.auth_token"),
                    );
                }
                if !(auth.eq_ignore_ascii_case("local") || auth.eq_ignore_ascii_case("token")) {
                    return ControlResponse::error(
                        id,
                        config_value_error("web.auth", "expected 'local' or 'token'"),
                    );
                }
                settings.web.auth = SmolStr::new(auth.to_ascii_lowercase());
                updated.push("web.auth");
                restart_required.push("web.auth");
            }
            "web.tls" => {
                settings.web.tls = parse_or_error!(expect_bool(key, value));
                updated.push("web.tls");
                restart_required.push("web.tls");
            }
            "discovery.enabled" => {
                settings.discovery.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("discovery.enabled");
                restart_required.push("discovery.enabled");
            }
            "discovery.service_name" => {
                let service_name = parse_or_error!(expect_non_empty_string(key, value));
                settings.discovery.service_name = SmolStr::new(service_name);
                updated.push("discovery.service_name");
                restart_required.push("discovery.service_name");
            }
            "discovery.advertise" => {
                settings.discovery.advertise = parse_or_error!(expect_bool(key, value));
                updated.push("discovery.advertise");
                restart_required.push("discovery.advertise");
            }
            "discovery.interfaces" => {
                settings.discovery.interfaces = parse_or_error!(expect_string_array(key, value))
                    .into_iter()
                    .map(SmolStr::new)
                    .collect();
                updated.push("discovery.interfaces");
                restart_required.push("discovery.interfaces");
            }
            "mesh.enabled" => {
                settings.mesh.enabled = parse_or_error!(expect_bool(key, value));
                updated.push("mesh.enabled");
                restart_required.push("mesh.enabled");
            }
            "mesh.role" => {
                let role = parse_or_error!(expect_non_empty_string(key, value));
                settings.mesh.role = parse_or_error!(
                    crate::config::MeshRole::parse(role)
                        .map_err(|err| config_value_error(key, &err.to_string()))
                );
                updated.push("mesh.role");
                restart_required.push("mesh.role");
            }
            "mesh.listen" => {
                let listen = parse_or_error!(expect_non_empty_string(key, value));
                settings.mesh.listen = SmolStr::new(listen);
                updated.push("mesh.listen");
                restart_required.push("mesh.listen");
            }
            "mesh.connect" => {
                settings.mesh.connect = parse_or_error!(expect_string_array(key, value))
                    .into_iter()
                    .map(SmolStr::new)
                    .collect();
                updated.push("mesh.connect");
                restart_required.push("mesh.connect");
            }
            "mesh.tls" => {
                settings.mesh.tls = parse_or_error!(expect_bool(key, value));
                updated.push("mesh.tls");
                restart_required.push("mesh.tls");
            }
            "mesh.publish" => {
                settings.mesh.publish = parse_or_error!(expect_string_array(key, value))
                    .into_iter()
                    .map(SmolStr::new)
                    .collect();
                updated.push("mesh.publish");
                restart_required.push("mesh.publish");
            }
            "mesh.subscribe" => {
                settings.mesh.subscribe = parse_or_error!(expect_string_map(key, value))
                    .into_iter()
                    .map(|(topic, alias)| (SmolStr::new(topic), SmolStr::new(alias)))
                    .collect();
                updated.push("mesh.subscribe");
                restart_required.push("mesh.subscribe");
            }
            "mesh.auth_token" => {
                if value.is_null() {
                    settings.mesh.auth_token = None;
                } else if let Some(token) = value.as_str() {
                    let token = token.trim();
                    if token.is_empty() {
                        return ControlResponse::error(
                            id,
                            config_value_error("mesh.auth_token", "must not be empty"),
                        );
                    }
                    settings.mesh.auth_token = Some(SmolStr::new(token));
                } else {
                    return ControlResponse::error(
                        id,
                        config_type_error("mesh.auth_token", "string or null"),
                    );
                }
                updated.push("mesh.auth_token");
                restart_required.push("mesh.auth_token");
            }
            "mesh.zenohd_version" => {
                let version = parse_or_error!(expect_non_empty_string(key, value));
                settings.mesh.zenohd_version = SmolStr::new(version);
                updated.push("mesh.zenohd_version");
                restart_required.push("mesh.zenohd_version");
            }
            "mesh.plugin_versions" => {
                settings.mesh.plugin_versions = parse_or_error!(expect_string_map(key, value))
                    .into_iter()
                    .map(|(name, version)| (SmolStr::new(name), SmolStr::new(version)))
                    .collect();
                updated.push("mesh.plugin_versions");
                restart_required.push("mesh.plugin_versions");
            }
            "runtime_cloud.profile" => {
                let profile = parse_or_error!(expect_non_empty_string(key, value));
                settings.runtime_cloud.profile =
                    parse_or_error!(RuntimeCloudProfile::parse(profile)
                        .map_err(|err| config_value_error(key, &err.to_string())));
                updated.push("runtime_cloud.profile");
                restart_required.push("runtime_cloud.profile");
            }
            "runtime_cloud.wan.allow_write" => {
                settings.runtime_cloud.wan_allow_write =
                    parse_or_error!(expect_wan_allow_write_rules(key, value));
                updated.push("runtime_cloud.wan.allow_write");
                restart_required.push("runtime_cloud.wan.allow_write");
            }
            "runtime_cloud.links.transports" => {
                settings.runtime_cloud.link_preferences =
                    parse_or_error!(expect_link_preference_rules(key, value));
                updated.push("runtime_cloud.links.transports");
                restart_required.push("runtime_cloud.links.transports");
            }
            "control.debug_enabled" => {
                debug_enabled = parse_or_error!(expect_bool(key, value));
                debug_enabled_changed = true;
                updated.push("control.debug_enabled");
            }
            "control.mode" => {
                let mode = parse_or_error!(expect_non_empty_string(key, value));
                control_mode = match mode.to_ascii_lowercase().as_str() {
                    "production" => ControlMode::Production,
                    "debug" => ControlMode::Debug,
                    _ => {
                        return ControlResponse::error(
                            id,
                            config_value_error("control.mode", "expected 'production' or 'debug'"),
                        )
                    }
                };
                control_mode_changed = true;
                updated.push("control.mode");
                restart_required.push("control.mode");
            }
            "runtime.execution_backend"
            | "runtime.execution_backend_source"
            | "realtime.profile"
            | "realtime.enabled"
            | "realtime.require_preempt_rt_kernel"
            | "realtime.lock_memory"
            | "realtime.scheduler"
            | "realtime.priority"
            | "realtime.cpu_affinity"
            | "realtime.strict" => {
                return ControlResponse::error(
                    id,
                    format!(
                        "{key} is startup-only; change it via runtime.toml/service posture and restart"
                    ),
                );
            }
            _ => {
                return ControlResponse::error(id, format!("unknown config key '{key}'"));
            }
        }
    }

    *settings_guard = settings.clone();

    if auth_changed {
        if let Ok(mut guard) = state.auth_token.lock() {
            *guard = auth_token;
        } else {
            return ControlResponse::error(id, "auth token unavailable".into());
        }
    }
    if control_mode_changed {
        if let Ok(mut guard) = state.control_mode.lock() {
            *guard = control_mode;
        } else {
            return ControlResponse::error(id, "control mode unavailable".into());
        }
    }
    if debug_enabled_changed {
        state.debug_enabled.store(debug_enabled, Ordering::Relaxed);
    }

    let _ = state
        .resource
        .send_command(crate::scheduler::ResourceCommand::UpdateWatchdog(
            settings_guard.watchdog,
        ));
    let _ = state
        .resource
        .send_command(crate::scheduler::ResourceCommand::UpdateFaultPolicy(
            settings_guard.fault_policy,
        ));
    let _ =
        state
            .resource
            .send_command(crate::scheduler::ResourceCommand::UpdateRetainSaveInterval(
                settings_guard.retain_save_interval,
            ));

    ControlResponse::ok(
        id,
        json!({ "updated": updated, "restart_required": restart_required }),
    )
}
