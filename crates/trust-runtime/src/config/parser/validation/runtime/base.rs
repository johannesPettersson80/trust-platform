fn validate_base_sections(cfg: &RuntimeToml) -> Result<(), RuntimeError> {
    if cfg.bundle.version != 1 {
        return Err(RuntimeError::InvalidConfig(
            format!("unsupported bundle.version {}", cfg.bundle.version).into(),
        ));
    }
    if cfg.resource.name.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "resource.name must not be empty".into(),
        ));
    }
    if cfg.resource.cycle_interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "resource.cycle_interval_ms must be >= 1".into(),
        ));
    }
    if cfg.runtime.control.endpoint.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.control.endpoint must not be empty".into(),
        ));
    }
    let log_level = cfg.runtime.log.level.trim().to_ascii_lowercase();
    if log_level.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.log.level must not be empty".into(),
        ));
    }
    if !matches!(
        log_level.as_str(),
        "error" | "warn" | "warning" | "info" | "debug" | "trace"
    ) {
        return Err(RuntimeError::InvalidConfig(
            "runtime.log.level must be error, warn, warning, info, debug, or trace".into(),
        ));
    }
    if cfg.runtime.retain.save_interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.retain.save_interval_ms must be >= 1".into(),
        ));
    }
    if cfg.runtime.watchdog.timeout_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.watchdog.timeout_ms must be >= 1".into(),
        ));
    }
    Ok(())
}

fn parse_tasks(tasks: Option<Vec<TaskSection>>) -> Result<Option<Vec<TaskOverride>>, RuntimeError> {
    tasks
        .map(|tasks| {
            tasks
                .into_iter()
                .map(parse_task)
                .collect::<Result<Vec<_>, RuntimeError>>()
        })
        .transpose()
}

fn parse_task(task: TaskSection) -> Result<TaskOverride, RuntimeError> {
    if task.name.trim().is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "resource.tasks[].name must not be empty".into(),
        ));
    }
    if task.interval_ms == 0 {
        return Err(RuntimeError::InvalidConfig(
            "resource.tasks[].interval_ms must be >= 1".into(),
        ));
    }
    if task.programs.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "resource.tasks[].programs must not be empty".into(),
        ));
    }
    if task
        .programs
        .iter()
        .any(|program| program.trim().is_empty())
    {
        return Err(RuntimeError::InvalidConfig(
            "resource.tasks[].programs entries must not be empty".into(),
        ));
    }
    let single = task
        .single
        .map(|value| parse_nonempty_entry(value, "resource.tasks[].single"))
        .transpose()?
        .map(SmolStr::new);

    Ok(TaskOverride {
        name: SmolStr::new(task.name),
        interval: parse_runtime_duration_millis(
            task.interval_ms,
            1,
            "resource.tasks[].interval_ms",
        )?,
        priority: task.priority,
        programs: task.programs.into_iter().map(SmolStr::new).collect(),
        single,
    })
}

fn parse_control(control: &ControlSection) -> Result<ParsedControl, RuntimeError> {
    let auth_token = control
        .auth_token
        .clone()
        .map(|value| parse_nonempty_entry(value, "runtime.control.auth_token"))
        .transpose()?
        .map(SmolStr::new);
    let mode = ControlMode::parse(control.mode.as_deref().unwrap_or("production"))?;
    let debug_enabled = match control.debug_enabled {
        Some(value) => value,
        None => matches!(mode, ControlMode::Debug),
    };
    if control.endpoint.starts_with("tcp://") && auth_token.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.control.auth_token required for tcp endpoint".into(),
        ));
    }
    Ok(ParsedControl {
        auth_token,
        mode,
        debug_enabled,
    })
}
