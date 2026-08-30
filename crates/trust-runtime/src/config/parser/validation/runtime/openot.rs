fn parse_openot_section(section: Option<OpenOtSection>) -> Result<ParsedOpenOt, RuntimeError> {
    let Some(section) = section else {
        return Ok(ParsedOpenOt {
            config: OpenOtTelemetryConfig::default(),
        });
    };

    let persistence_config = parse_openot_persistence(section.persistence)?;
    let persistence_enabled = persistence_config.enabled;
    let enabled = section.enabled.unwrap_or(false);
    if persistence_enabled && !enabled {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.enabled=true requires runtime.openot.enabled=true"
                .into(),
        ));
    }
    let path = parse_optional_path("runtime.openot.path", section.path)?;
    if enabled && path.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.path must not be empty when runtime.openot.enabled=true".into(),
        ));
    }

    let capacity = section.capacity.unwrap_or(4096);
    if capacity == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.capacity must be >= 1".into(),
        ));
    }

    let fence_mode = match section.fence_mode.as_deref() {
        Some(value) => OpenOtTelemetryFenceMode::parse(value)?,
        None => OpenOtTelemetryFenceMode::Fenced,
    };
    let allow_unfenced_for_proof = section.allow_unfenced_for_proof.unwrap_or(false);
    if fence_mode == OpenOtTelemetryFenceMode::Unfenced && !allow_unfenced_for_proof {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.fence_mode='unfenced' requires runtime.openot.allow_unfenced_for_proof=true"
                .into(),
        ));
    }

    let source = match section.source.as_deref() {
        Some(value) => OpenOtTelemetrySource::parse(value)?,
        None => OpenOtTelemetrySource::Heartbeat,
    };
    let producer_instance = section
        .producer_instance
        .map(|value| parse_nonempty_entry(value, "runtime.openot.producer_instance"))
        .transpose()?
        .map(SmolStr::new);
    let configured_instances = section.producer_instances.unwrap_or_default();
    let producer_instances = configured_instances
        .into_iter()
        .map(|value| {
            parse_nonempty_entry(value, "runtime.openot.producer_instances").map(SmolStr::new)
        })
        .collect::<Result<Vec<_>, RuntimeError>>()?;

    if producer_instance.is_some() && !producer_instances.is_empty() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.producer_instance and runtime.openot.producer_instances are aliases; set only one"
                .into(),
        ));
    }
    let normalized_producer_instances = if producer_instances.is_empty() {
        producer_instance.iter().cloned().collect::<Vec<_>>()
    } else {
        producer_instances
    };
    let mut seen_instances = std::collections::BTreeSet::<String>::new();
    for path in &normalized_producer_instances {
        if !seen_instances.insert(path.to_string()) {
            return Err(RuntimeError::InvalidConfig(
                format!("runtime.openot.producer_instances contains duplicate path '{path}'")
                    .into(),
            ));
        }
    }

    match source {
        OpenOtTelemetrySource::Heartbeat => {
            if !normalized_producer_instances.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance(s) are only valid when runtime.openot.source='st-fb'"
                        .into(),
                ));
            }
        }
        OpenOtTelemetrySource::StFb => {
            if normalized_producer_instances.is_empty() {
                return Err(RuntimeError::InvalidConfig(
                    "runtime.openot.producer_instance or runtime.openot.producer_instances is required when runtime.openot.source='st-fb'"
                        .into(),
                ));
            }
            for path in &normalized_producer_instances {
                if !is_qualified_openot_producer_path(path.as_str()) {
                    return Err(RuntimeError::InvalidConfig(
                        "runtime.openot.producer_instance(s) must be qualified paths like 'Main.Producer'"
                            .into(),
                    ));
                }
            }
        }
    }

    Ok(ParsedOpenOt {
        config: OpenOtTelemetryConfig {
            enabled,
            path: path.unwrap_or_default(),
            capacity,
            fence_mode,
            allow_unfenced_for_proof,
            source,
            producer_instance,
            producer_instances: normalized_producer_instances,
            persistence: persistence_config,
        },
    })
}

fn parse_openot_persistence(
    persistence: Option<OpenOtPersistenceSection>,
) -> Result<OpenOtPersistenceConfig, RuntimeError> {
    let persistence_enabled = persistence
        .as_ref()
        .and_then(|persistence| persistence.enabled)
        .unwrap_or(false);
    let persistence_backend = persistence
        .as_ref()
        .and_then(|persistence| persistence.backend.as_deref())
        .map(OpenOtPersistenceBackend::parse)
        .transpose()?;
    if persistence_enabled && persistence_backend.is_none() {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.backend is required when runtime.openot.persistence.enabled=true"
                .into(),
        ));
    }
    let batch_size = persistence
        .as_ref()
        .and_then(|persistence| persistence.batch_size)
        .unwrap_or(256);
    let flush_interval_ms = persistence
        .as_ref()
        .and_then(|persistence| persistence.flush_interval_ms)
        .unwrap_or(250);
    let queue_capacity = persistence
        .as_ref()
        .and_then(|persistence| persistence.queue_capacity)
        .unwrap_or(4096);
    let shutdown_timeout_ms = persistence
        .as_ref()
        .and_then(|persistence| persistence.shutdown_timeout_ms)
        .unwrap_or(5000);
    let retry_initial_ms = persistence
        .as_ref()
        .and_then(|persistence| persistence.retry_initial_ms)
        .unwrap_or(250);
    let retry_max_ms = persistence
        .as_ref()
        .and_then(|persistence| persistence.retry_max_ms)
        .unwrap_or(30000);
    let retry_multiplier = persistence
        .as_ref()
        .and_then(|persistence| persistence.retry_multiplier)
        .unwrap_or(2);
    let retry_max_attempts = persistence
        .as_ref()
        .and_then(|persistence| persistence.retry_max_attempts)
        .unwrap_or(20);
    if retry_max_attempts == 0 {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.retry_max_attempts must be >= 1".into(),
        ));
    }
    for (key, value) in [
        ("batch_size", batch_size),
        ("queue_capacity", queue_capacity),
    ] {
        if value == 0 {
            return Err(RuntimeError::InvalidConfig(
                format!("runtime.openot.persistence.{key} must be >= 1").into(),
            ));
        }
    }
    for (key, value) in [
        ("flush_interval_ms", flush_interval_ms),
        ("shutdown_timeout_ms", shutdown_timeout_ms),
        ("retry_initial_ms", retry_initial_ms),
        ("retry_max_ms", retry_max_ms),
    ] {
        if value == 0 {
            return Err(RuntimeError::InvalidConfig(
                format!("runtime.openot.persistence.{key} must be >= 1").into(),
            ));
        }
    }
    if batch_size > queue_capacity {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.batch_size must not exceed queue_capacity".into(),
        ));
    }
    if retry_initial_ms > retry_max_ms {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.retry_initial_ms must not exceed retry_max_ms".into(),
        ));
    }
    if !(1..=16).contains(&retry_multiplier) {
        return Err(RuntimeError::InvalidConfig(
            "runtime.openot.persistence.retry_multiplier must be between 1 and 16".into(),
        ));
    }
    let sqlite = persistence
        .as_ref()
        .and_then(|persistence| persistence.sqlite.as_ref())
        .map(|sqlite| -> Result<OpenOtSqlitePersistenceConfig, RuntimeError> {
            let path = parse_optional_path(
                "runtime.openot.persistence.sqlite.path",
                sqlite.path.clone(),
            )?
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    "runtime.openot.persistence.sqlite.path is required".into(),
                )
            })?;
            Ok(OpenOtSqlitePersistenceConfig { path })
        })
        .transpose()?;
    let postgresql = persistence
        .as_ref()
        .and_then(|persistence| persistence.postgresql.as_ref())
        .map(|section| parse_postgresql_persistence(section, "postgresql"))
        .transpose()?;
    let timescaledb = persistence
        .as_ref()
        .and_then(|persistence| persistence.timescaledb.as_ref())
        .map(parse_timescaledb_persistence)
        .transpose()?;
    let mysql = persistence
        .as_ref()
        .and_then(|persistence| persistence.mysql.as_ref())
        .map(parse_mysql_persistence)
        .transpose()?;
    let sqlserver = persistence
        .as_ref()
        .and_then(|persistence| persistence.sqlserver.as_ref())
        .map(|section| parse_sqlserver_persistence(section, "sqlserver"))
        .transpose()?;
    let influxdb3 = persistence
        .as_ref()
        .and_then(|persistence| persistence.influxdb3.as_ref())
        .map(parse_influxdb3_persistence)
        .transpose()?;

    for (backend, present) in [
        (OpenOtPersistenceBackend::Sqlite, sqlite.is_some()),
        (OpenOtPersistenceBackend::PostgreSql, postgresql.is_some()),
        (OpenOtPersistenceBackend::TimescaleDb, timescaledb.is_some()),
        (OpenOtPersistenceBackend::MySql, mysql.is_some()),
        (OpenOtPersistenceBackend::SqlServer, sqlserver.is_some()),
        (OpenOtPersistenceBackend::InfluxDb3, influxdb3.is_some()),
    ] {
        if present && persistence_backend != Some(backend) {
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "unselected runtime.openot.persistence.{} table is not allowed",
                    backend.as_str()
                )
                .into(),
            ));
        }
    }

    if persistence_enabled {
        let selected_table_present = match persistence_backend {
            Some(OpenOtPersistenceBackend::Sqlite) => sqlite.is_some(),
            Some(OpenOtPersistenceBackend::PostgreSql) => postgresql.is_some(),
            Some(OpenOtPersistenceBackend::TimescaleDb) => timescaledb.is_some(),
            Some(OpenOtPersistenceBackend::MySql) => mysql.is_some(),
            Some(OpenOtPersistenceBackend::SqlServer) => sqlserver.is_some(),
            Some(OpenOtPersistenceBackend::InfluxDb3) => influxdb3.is_some(),
            None => false,
        };
        if !selected_table_present {
            let backend = persistence_backend.expect("enabled backend checked above");
            return Err(RuntimeError::InvalidConfig(
                format!(
                    "runtime.openot.persistence.{} is required when backend='{}'",
                    backend.as_str(),
                    backend.as_str()
                )
                .into(),
            ));
        }
    }
    Ok(OpenOtPersistenceConfig {
        enabled: persistence_enabled,
        backend: persistence_backend,
        batch_size,
        flush_interval_ms,
        queue_capacity,
        shutdown_timeout_ms,
        retry_initial_ms,
        retry_max_ms,
        retry_multiplier,
        retry_max_attempts,
        sqlite,
        postgresql,
        timescaledb,
        mysql,
        sqlserver,
        influxdb3,
    })
}
fn required_persistence_text(
    value: &Option<String>,
    key: &str,
) -> Result<SmolStr, RuntimeError> {
    value
        .as_ref()
        .map(|value| parse_nonempty_entry(value.clone(), key).map(SmolStr::new))
        .transpose()?
        .ok_or_else(|| RuntimeError::InvalidConfig(format!("{key} is required").into()))
}

fn required_persistence_tls(
    value: &Option<String>,
    key: &str,
) -> Result<OpenOtPersistenceTlsMode, RuntimeError> {
    let value = required_persistence_text(value, key)?;
    OpenOtPersistenceTlsMode::parse(value.as_str()).map_err(|_| {
        RuntimeError::InvalidConfig(format!("invalid {key} '{value}'").into())
    })
}

fn required_persistence_env_name(
    value: &Option<String>,
    key: &str,
) -> Result<SmolStr, RuntimeError> {
    let value = required_persistence_text(value, key)?;
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !valid {
        return Err(RuntimeError::InvalidConfig(
            format!("{key} must be an environment variable name").into(),
        ));
    }
    Ok(value)
}

fn required_sql_identifier(
    value: &Option<String>,
    key: &str,
) -> Result<SmolStr, RuntimeError> {
    let value = required_persistence_text(value, key)?;
    let mut chars = value.chars();
    let valid = chars
        .next()
        .is_some_and(|first| first == '_' || first.is_ascii_alphabetic())
        && chars.all(|character| character == '_' || character.is_ascii_alphanumeric());
    if !valid {
        return Err(RuntimeError::InvalidConfig(
            format!("{key} must be a SQL identifier").into(),
        ));
    }
    Ok(value)
}

fn optional_persistence_path(
    value: &Option<String>,
    key: &str,
) -> Result<Option<PathBuf>, RuntimeError> {
    parse_optional_path(key, value.clone())
}

fn parse_postgresql_persistence(
    section: &OpenOtRemoteSqlPersistenceSection,
    table: &str,
) -> Result<OpenOtPostgreSqlPersistenceConfig, RuntimeError> {
    let prefix = format!("runtime.openot.persistence.{table}");
    Ok(OpenOtPostgreSqlPersistenceConfig {
        connection_url_env: required_persistence_env_name(
            &section.connection_url_env,
            &format!("{prefix}.connection_url_env"),
        )?,
        schema: required_sql_identifier(&section.schema, &format!("{prefix}.schema"))?,
        tls: required_persistence_tls(&section.tls, &format!("{prefix}.tls"))?,
        ca_cert_path: optional_persistence_path(
            &section.ca_cert_path,
            &format!("{prefix}.ca_cert_path"),
        )?,
    })
}

fn parse_timescaledb_persistence(
    section: &OpenOtRemoteSqlPersistenceSection,
) -> Result<OpenOtTimescaleDbPersistenceConfig, RuntimeError> {
    let parsed = parse_postgresql_persistence(section, "timescaledb")?;
    Ok(OpenOtTimescaleDbPersistenceConfig {
        connection_url_env: parsed.connection_url_env,
        schema: parsed.schema,
        tls: parsed.tls,
        ca_cert_path: parsed.ca_cert_path,
    })
}

fn parse_mysql_persistence(
    section: &OpenOtMySqlPersistenceSection,
) -> Result<OpenOtMySqlPersistenceConfig, RuntimeError> {
    let prefix = "runtime.openot.persistence.mysql";
    Ok(OpenOtMySqlPersistenceConfig {
        connection_url_env: required_persistence_env_name(
            &section.connection_url_env,
            &format!("{prefix}.connection_url_env"),
        )?,
        database: required_sql_identifier(&section.database, &format!("{prefix}.database"))?,
        tls: required_persistence_tls(&section.tls, &format!("{prefix}.tls"))?,
        ca_cert_path: optional_persistence_path(
            &section.ca_cert_path,
            &format!("{prefix}.ca_cert_path"),
        )?,
    })
}

fn parse_sqlserver_persistence(
    section: &OpenOtRemoteSqlPersistenceSection,
    table: &str,
) -> Result<OpenOtSqlServerPersistenceConfig, RuntimeError> {
    let parsed = parse_postgresql_persistence(section, table)?;
    Ok(OpenOtSqlServerPersistenceConfig {
        connection_url_env: parsed.connection_url_env,
        schema: parsed.schema,
        tls: parsed.tls,
        ca_cert_path: parsed.ca_cert_path,
    })
}

fn parse_influxdb3_persistence(
    section: &OpenOtInfluxDb3PersistenceSection,
) -> Result<OpenOtInfluxDb3PersistenceConfig, RuntimeError> {
    let prefix = "runtime.openot.persistence.influxdb3";
    let spool_path = parse_optional_path(
        &format!("{prefix}.spool_path"),
        section.spool_path.clone(),
    )?
    .ok_or_else(|| {
        RuntimeError::InvalidConfig(format!("{prefix}.spool_path is required").into())
    })?;
    Ok(OpenOtInfluxDb3PersistenceConfig {
        host_env: required_persistence_env_name(&section.host_env, &format!("{prefix}.host_env"))?,
        token_env: required_persistence_env_name(
            &section.token_env,
            &format!("{prefix}.token_env"),
        )?,
        database: required_persistence_text(&section.database, &format!("{prefix}.database"))?,
        spool_path,
        max_bytes: section.max_bytes.filter(|value| *value > 0).ok_or_else(|| {
            RuntimeError::InvalidConfig(format!("{prefix}.max_bytes must be greater than zero").into())
        })?,
        ca_cert_path: parse_optional_path(
            &format!("{prefix}.ca_cert_path"),
            section.ca_cert_path.clone(),
        )?,
    })
}

fn is_qualified_openot_producer_path(path: &str) -> bool {
    let mut parts = path.split('.');
    let Some(program) = parts.next() else {
        return false;
    };
    let Some(instance) = parts.next() else {
        return false;
    };
    parts.next().is_none()
        && !program.trim().is_empty()
        && !instance.trim().is_empty()
        && program == program.trim()
        && instance == instance.trim()
}
