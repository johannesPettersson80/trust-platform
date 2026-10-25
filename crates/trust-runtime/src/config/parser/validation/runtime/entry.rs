impl RuntimeToml {
    pub(crate) fn into_config(self) -> Result<RuntimeConfig, RuntimeError> {
        validate_base_sections(&self)?;

        let bundle_version = self.bundle.version;
        let resource_name = SmolStr::new(self.resource.name);
        let cycle_interval = parse_runtime_duration_millis(
            self.resource.cycle_interval_ms,
            1,
            "resource.cycle_interval_ms",
        )?;
        let max_statements_per_scan = self
            .resource
            .max_statements_per_scan
            .and_then(|budget| u32::try_from(budget).ok())
            .unwrap_or(trust_runtime_core::deployment::HOSTED_MAX_STATEMENTS);
        let tasks = parse_tasks(self.resource.tasks)?;

        let RuntimeSection {
            execution_backend,
            control,
            log,
            events,
            retain,
            watchdog,
            fault,
            web,
            tls,
            deploy,
            discovery,
            mesh,
            cloud,
            realtime,
            openot,
            observability,
            hmi_persistence,
            ads,
            ads_server,
            opcua_client,
            opcua,
        } = self.runtime;
        // `runtime.execution_backend` selects nothing: every runtime runs the compiled program
        // (specification 44 §15). A project that names the bytecode engine's `vm` or `native` still
        // loads; any other value is refused as before.
        let execution_backend_source = match execution_backend.as_deref().map(str::trim) {
            Some(value)
                if value.eq_ignore_ascii_case("vm") || value.eq_ignore_ascii_case("native") =>
            {
                ExecutionBackendSource::Config
            }
            Some(value) if value.eq_ignore_ascii_case("interpreter") => {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.execution_backend='interpreter' is no longer supported; remove the key, every runtime runs the compiled program"
                        .into(),
                ));
            }
            Some("") => {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.execution_backend must not be empty".into(),
                ));
            }
            Some(value) => {
                return Err(RuntimeError::InvalidConfig(
                    format!("invalid runtime.execution_backend '{value}'").into(),
                ));
            }
            None => ExecutionBackendSource::Default,
        };

        let parsed_control = parse_control(&control)?;
        let parsed_retain_mode = parse_retain_mode(&retain)?;
        let retain_path = parse_optional_path("runtime.retain.path", retain.path)?;
        let watchdog_action = WatchdogAction::parse(&watchdog.action)?;
        let watchdog_hang_action = match watchdog.hang_action.as_deref() {
            Some(text) => HangAction::parse(text)?,
            None => HangAction::default(),
        };
        let fault_policy = FaultPolicy::parse(&fault.policy)?;
        let parsed_tls = parse_tls_section(tls)?;
        let parsed_web = parse_web_section(
            web,
            parsed_control.auth_token.as_ref(),
            parsed_tls.mode,
            parsed_tls.require_remote,
        )?;
        let parsed_deploy = parse_deploy_section(deploy)?;
        let parsed_discovery = parse_discovery_section(discovery)?;
        let parsed_mesh = parse_mesh_section(mesh, parsed_tls.mode, parsed_tls.require_remote)?;
        let parsed_cloud = parse_runtime_cloud_section(cloud)?;
        let parsed_realtime = parse_realtime_section(realtime)?;
        let parsed_openot = parse_openot_section(openot)?;
        let observability = parse_observability_section(observability)?;
        let hmi_persistence = parse_hmi_persistence_section(hmi_persistence)?;
        let ads = parse_ads_section(ads)?;
        let ads_server = parse_ads_server_section(ads_server)?;
        let opcua_client = parse_opcua_client_section(opcua_client)?;
        let opcua = parse_opcua_section(opcua)?;

        Ok(RuntimeConfig {
            bundle_version,
            resource_name,
            cycle_interval,
            max_statements_per_scan,
            execution_backend_source,
            control_endpoint: SmolStr::new(control.endpoint),
            control_auth_token: parsed_control.auth_token,
            control_debug_enabled: parsed_control.debug_enabled,
            control_mode: parsed_control.mode,
            log_level: SmolStr::new(log.level),
            event_capacity: events.and_then(|events| events.capacity),
            retain_mode: parsed_retain_mode,
            retain_path,
            retain_save_interval: parse_runtime_duration_millis(
                retain.save_interval_ms,
                1,
                "runtime.retain.save_interval_ms",
            )?,
            watchdog: WatchdogPolicy {
                enabled: watchdog.enabled,
                timeout: parse_runtime_duration_millis(
                    watchdog.timeout_ms,
                    1,
                    "runtime.watchdog.timeout_ms",
                )?,
                action: watchdog_action,
            },
            watchdog_hang_action,
            fault_policy,
            web: parsed_web.config,
            tls: parsed_tls.config,
            deploy: parsed_deploy.config,
            discovery: parsed_discovery.config,
            mesh: parsed_mesh.config,
            runtime_cloud_profile: parsed_cloud.profile,
            runtime_cloud_wan_allow_write: parsed_cloud.wan_allow_write,
            runtime_cloud_link_preferences: parsed_cloud.link_preferences,
            realtime: parsed_realtime.config,
            openot: parsed_openot.config,
            observability,
            hmi_persistence,
            ads,
            ads_server,
            opcua_client,
            opcua,
            tasks,
        })
    }
}
