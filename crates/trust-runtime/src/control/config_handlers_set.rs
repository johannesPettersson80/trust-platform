pub(super) fn handle_config_set(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
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
    // Every lock is taken before the first commit, and each commit writes through a guard that is
    // already held. A failed lock or a refused scheduler update therefore changes no setting
    // (specification 11 section 6.8.1).
    let mut auth_token_guard = match state.auth_token.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "auth token unavailable".into()),
    };
    let mut control_mode_guard = match state.control_mode.lock() {
        Ok(guard) => guard,
        Err(_) => return ControlResponse::error(id, "control mode unavailable".into()),
    };
    let shared = match shared_settings(
        &settings,
        *control_mode_guard,
        state.debug_enabled.load(Ordering::Relaxed),
    ) {
        Ok(shared) => shared,
        Err(error) => return ControlResponse::error(id, error),
    };

    // The keys every runtime has are staged by the shared rules; the computer stages its own
    // services' keys on its copy of the settings.
    let token_present = auth_token_guard.is_some();
    let mut staged = StagedSettings::new(shared);
    if let Err(error) = shared::stage_request(
        params,
        &mut staged,
        state.control_requires_auth,
        |key, value, staged| stage_host_key(key, value, &mut settings, staged, token_present),
    ) {
        return ControlResponse::error(id, error);
    }
    let token_after = staged.token.leaves_token(token_present);
    if settings.web.auth.eq_ignore_ascii_case("token") && !token_after {
        return ControlResponse::error(
            id,
            config_value_error("web.auth", "token mode requires control.auth_token"),
        );
    }
    apply_shared(&staged.settings, &mut settings);

    // The scheduler takes its policy set as one command, so a full queue refuses all of it or none.
    // A request that changes no scheduler policy sends nothing, so a full queue cannot refuse it.
    if let Some(policies) = staged.changed_policies()
        && let Err(error) =
            state
                .resource
                .send_command(crate::scheduler::ResourceCommand::UpdateSchedulerPolicies(
                    policies,
                ))
    {
        return super::resource_queue::queue_refusal(id, error, state);
    }

    // Every fallible step has run. The commits below write only through guards already held, so
    // they cannot fail and no setting changes unless the whole request succeeds.
    *settings_guard = settings;
    staged.token.apply_to(&mut auth_token_guard);
    *control_mode_guard = match staged.settings.control_mode {
        SharedControlMode::Production => ControlMode::Production,
        SharedControlMode::Debug => ControlMode::Debug,
    };
    state
        .debug_enabled
        .store(staged.settings.debug_enabled, Ordering::Relaxed);

    ControlResponse::ok(
        id,
        json!({ "updated": staged.updated, "restart_required": staged.restart_required }),
    )
}

/// Writes the settings every runtime has, as a request staged them, into the computer's settings.
/// The control mode and the debug surface are kept apart and committed by the caller.
fn apply_shared(shared: &SharedSettings, settings: &mut RuntimeSettings) {
    settings.log_level = SmolStr::new_static(shared.log_level.as_str());
    settings.watchdog = shared.watchdog;
    settings.watchdog_hang_action = match shared.hang_action {
        SharedHangAction::Fault => crate::watchdog::HangAction::Fault,
        SharedHangAction::Abort => crate::watchdog::HangAction::Abort,
    };
    settings.fault_policy = shared.fault_policy;
    settings.retain_mode = shared.retain_mode;
    settings.retain_save_interval = shared.retain_save_interval;
}

/// Stages `key` := `value`, a key of the computer's own services, on `settings`; none for a key the
/// computer does not have. Each of them takes effect when its service starts again. `token_present`
/// says whether the control token is set before the request, which a token the request sets
/// replaces.
fn stage_host_key(
    key: &str,
    value: &serde_json::Value,
    settings: &mut RuntimeSettings,
    staged: &mut StagedSettings,
    token_present: bool,
) -> Option<Result<(), String>> {
    macro_rules! stage {
        ($expr:expr) => {
            match $expr {
                Ok(value) => value,
                Err(error) => return Some(Err(error)),
            }
        };
    }

    let name: &'static str = match key {
        "web.enabled" => {
            settings.web.enabled = stage!(expect_bool(key, value));
            "web.enabled"
        }
        "web.listen" => {
            let listen = stage!(expect_non_empty_string(key, value));
            settings.web.listen = SmolStr::new(listen);
            "web.listen"
        }
        "web.auth" => {
            let auth = stage!(expect_non_empty_string(key, value));
            let token = staged.token.leaves_token(token_present);
            if auth.eq_ignore_ascii_case("token") && !token {
                return Some(Err(config_value_error(
                    "web.auth",
                    "token mode requires control.auth_token",
                )));
            }
            if !(auth.eq_ignore_ascii_case("local") || auth.eq_ignore_ascii_case("token")) {
                return Some(Err(config_value_error(
                    "web.auth",
                    "expected 'local' or 'token'",
                )));
            }
            settings.web.auth = SmolStr::new(auth.to_ascii_lowercase());
            "web.auth"
        }
        "web.tls" => {
            settings.web.tls = stage!(expect_bool(key, value));
            "web.tls"
        }
        "discovery.enabled" => {
            settings.discovery.enabled = stage!(expect_bool(key, value));
            "discovery.enabled"
        }
        "discovery.service_name" => {
            let service_name = stage!(expect_non_empty_string(key, value));
            settings.discovery.service_name = SmolStr::new(service_name);
            "discovery.service_name"
        }
        "discovery.advertise" => {
            settings.discovery.advertise = stage!(expect_bool(key, value));
            "discovery.advertise"
        }
        "discovery.interfaces" => {
            settings.discovery.interfaces = stage!(expect_string_array(key, value))
                .into_iter()
                .map(SmolStr::new)
                .collect();
            "discovery.interfaces"
        }
        "discovery.host_group" => {
            if value.is_null() {
                settings.discovery.host_group = None;
            } else {
                // The same trimming and non-empty rule as `runtime.toml`'s
                // `runtime.discovery.host_group` parser (specification 11, runtime discovery).
                let host_group = stage!(expect_non_empty_string(key, value));
                settings.discovery.host_group = Some(SmolStr::new(host_group));
            }
            "discovery.host_group"
        }
        "mesh.enabled" => {
            settings.mesh.enabled = stage!(expect_bool(key, value));
            "mesh.enabled"
        }
        "mesh.role" => {
            let role = stage!(expect_non_empty_string(key, value));
            settings.mesh.role = stage!(crate::config::MeshRole::parse(role)
                .map_err(|err| config_value_error(key, &err.to_string())));
            "mesh.role"
        }
        "mesh.listen" => {
            let listen = stage!(expect_non_empty_string(key, value));
            settings.mesh.listen = SmolStr::new(listen);
            "mesh.listen"
        }
        "mesh.connect" => {
            settings.mesh.connect = stage!(expect_string_array(key, value))
                .into_iter()
                .map(SmolStr::new)
                .collect();
            "mesh.connect"
        }
        "mesh.tls" => {
            settings.mesh.tls = stage!(expect_bool(key, value));
            "mesh.tls"
        }
        "mesh.publish" => {
            settings.mesh.publish = stage!(expect_string_array(key, value))
                .into_iter()
                .map(SmolStr::new)
                .collect();
            "mesh.publish"
        }
        "mesh.subscribe" => {
            settings.mesh.subscribe = stage!(expect_string_map(key, value))
                .into_iter()
                .map(|(topic, alias)| (SmolStr::new(topic), SmolStr::new(alias)))
                .collect();
            "mesh.subscribe"
        }
        "mesh.auth_token" => {
            if value.is_null() {
                settings.mesh.auth_token = None;
            } else if let Some(token) = value.as_str() {
                let token = token.trim();
                if token.is_empty() {
                    return Some(Err(config_value_error(
                        "mesh.auth_token",
                        "must not be empty",
                    )));
                }
                settings.mesh.auth_token = Some(SmolStr::new(token));
            } else {
                return Some(Err(config_type_error("mesh.auth_token", "string or null")));
            }
            "mesh.auth_token"
        }
        "mesh.zenohd_version" => {
            let version = stage!(expect_non_empty_string(key, value));
            settings.mesh.zenohd_version = SmolStr::new(version);
            "mesh.zenohd_version"
        }
        "mesh.plugin_versions" => {
            settings.mesh.plugin_versions = stage!(expect_string_map(key, value))
                .into_iter()
                .map(|(name, version)| (SmolStr::new(name), SmolStr::new(version)))
                .collect();
            "mesh.plugin_versions"
        }
        "runtime_cloud.profile" => {
            let profile = stage!(expect_non_empty_string(key, value));
            settings.runtime_cloud.profile = stage!(RuntimeCloudProfile::parse(profile)
                .map_err(|err| config_value_error(key, &err.to_string())));
            "runtime_cloud.profile"
        }
        "runtime_cloud.wan.allow_write" => {
            settings.runtime_cloud.wan_allow_write =
                stage!(expect_wan_allow_write_rules(key, value));
            "runtime_cloud.wan.allow_write"
        }
        "runtime_cloud.links.transports" => {
            settings.runtime_cloud.link_preferences =
                stage!(expect_link_preference_rules(key, value));
            "runtime_cloud.links.transports"
        }
        "realtime.profile"
        | "realtime.enabled"
        | "realtime.require_preempt_rt_kernel"
        | "realtime.lock_memory"
        | "realtime.scheduler"
        | "realtime.priority"
        | "realtime.cpu_affinity"
        | "realtime.strict" => {
            return Some(Err(format!(
                "{key} is startup-only; change it via runtime.toml/service posture and restart"
            )));
        }
        _ => return None,
    };
    staged.updated.push(name);
    staged.restart_required.push(name);
    Some(Ok(()))
}
