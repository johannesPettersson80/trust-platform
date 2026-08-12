//! Setup, IO configuration, and source/hmi asset helper functions.

#![allow(missing_docs)]

use super::*;

pub(super) fn default_bundle_root(bundle_root: &Option<PathBuf>) -> PathBuf {
    bundle_root
        .clone()
        .or_else(|| std::env::current_dir().ok())
        .unwrap_or_else(|| PathBuf::from("."))
}

fn default_resource_name(bundle_root: &Path) -> SmolStr {
    let Some(project_name) = bundle_root.file_name().and_then(|name| name.to_str()) else {
        return SmolStr::new_static("trust-plc");
    };
    SmolStr::new(project_name.replace(|c: char| !c.is_ascii_alphanumeric(), "_"))
}

fn detect_default_driver() -> String {
    if crate::setup::is_raspberry_pi_hint() {
        "gpio".to_string()
    } else {
        "loopback".to_string()
    }
}

pub(super) fn setup_defaults(bundle_root: &Option<PathBuf>) -> SetupDefaultsResponse {
    let project_path = default_bundle_root(bundle_root);
    let runtime_path = project_path.join("runtime.toml");
    let io_path = project_path.join("io.toml");

    let runtime_loaded = if runtime_path.exists() {
        RuntimeConfig::load(&runtime_path).ok()
    } else {
        None
    };
    let (resource_name, cycle_ms) = if let Some(runtime) = runtime_loaded.as_ref() {
        (
            runtime.resource_name.to_string(),
            runtime.cycle_interval.as_millis() as u64,
        )
    } else {
        (default_resource_name(&project_path).to_string(), 100)
    };

    let system_io = load_system_io_config().ok().flatten();
    let system_io_exists = system_io.is_some();

    let (driver, use_system_io) = if io_path.exists() {
        match IoConfig::load(&io_path) {
            Ok(io) => (
                io.drivers
                    .first()
                    .map(|driver| driver.name.to_string())
                    .unwrap_or_else(detect_default_driver),
                false,
            ),
            Err(_) => (detect_default_driver(), system_io_exists),
        }
    } else if let Some(system_io) = system_io {
        (
            system_io
                .drivers
                .first()
                .map(|driver| driver.name.to_string())
                .unwrap_or_else(detect_default_driver),
            true,
        )
    } else {
        (detect_default_driver(), false)
    };

    let write_system_io = !system_io_exists;
    let needs_setup = runtime_loaded.is_none() || (!io_path.exists() && !system_io_exists);

    SetupDefaultsResponse {
        project_path: project_path.display().to_string(),
        resource_name,
        cycle_ms,
        driver,
        supported_drivers: IoDriverRegistry::default_registry().canonical_driver_names(),
        use_system_io,
        system_io_exists,
        write_system_io,
        needs_setup,
    }
}

fn json_to_toml(value: &serde_json::Value) -> toml::Value {
    match value {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(value) => toml::Value::Boolean(*value),
        serde_json::Value::Number(value) => {
            if let Some(i) = value.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(u) = value.as_u64() {
                toml::Value::Integer(u as i64)
            } else if let Some(f) = value.as_f64() {
                toml::Value::Float(f)
            } else {
                toml::Value::String(value.to_string())
            }
        }
        serde_json::Value::String(value) => toml::Value::String(value.clone()),
        serde_json::Value::Array(values) => {
            toml::Value::Array(values.iter().map(json_to_toml).collect())
        }
        serde_json::Value::Object(values) => {
            let mut table = toml::map::Map::new();
            for (key, value) in values {
                table.insert(key.clone(), json_to_toml(value));
            }
            toml::Value::Table(table)
        }
    }
}

pub(super) fn io_config_to_response(
    config: IoConfig,
    source: &str,
    use_system_io: bool,
) -> IoConfigResponse {
    let drivers = config
        .drivers
        .iter()
        .map(|driver| IoDriverConfigResponse {
            name: driver.name.to_string(),
            params: serde_json::to_value(&driver.params).unwrap_or_else(|_| json!({})),
        })
        .collect::<Vec<_>>();
    let primary = drivers.first().cloned().unwrap_or(IoDriverConfigResponse {
        name: detect_default_driver(),
        params: json!({}),
    });
    let safe_state = config
        .safe_state
        .outputs
        .iter()
        .map(|(address, value)| IoSafeStateEntry {
            address: format_io_address(address),
            value: format_io_safe_state_value(value),
        })
        .collect::<Vec<_>>();
    IoConfigResponse {
        driver: primary.name,
        params: primary.params,
        drivers,
        safe_state,
        supported_drivers: IoDriverRegistry::default_registry().canonical_driver_names(),
        source: source.to_string(),
        use_system_io,
    }
}

pub(super) fn load_io_config(
    bundle_root: &Option<PathBuf>,
) -> Result<IoConfigResponse, RuntimeError> {
    let project_root = default_bundle_root(bundle_root);
    let project_io = project_root.join("io.toml");
    if project_io.is_file() {
        let config = IoConfig::load(&project_io)?;
        return Ok(io_config_to_response(config, "project", false));
    }
    if let Some(system) = load_system_io_config().ok().flatten() {
        return Ok(io_config_to_response(system, "system", true));
    }
    Ok(IoConfigResponse {
        driver: detect_default_driver(),
        params: json!({}),
        drivers: vec![IoDriverConfigResponse {
            name: detect_default_driver(),
            params: json!({}),
        }],
        safe_state: Vec::new(),
        supported_drivers: IoDriverRegistry::default_registry().canonical_driver_names(),
        source: "default".to_string(),
        use_system_io: false,
    })
}

pub(super) fn save_io_config(
    bundle_root: &Option<PathBuf>,
    payload: &IoConfigRequest,
) -> Result<String, RuntimeError> {
    let project_root = default_bundle_root(bundle_root);
    let io_path = project_root.join("io.toml");
    let use_system = payload.use_system_io.unwrap_or(false);
    if use_system {
        if io_path.exists() {
            std::fs::remove_file(&io_path).map_err(|err| {
                RuntimeError::ControlError(format!("failed to remove io.toml: {err}").into())
            })?;
        }
        return Ok("✓ Using system I/O config. Restart the runtime to apply.".to_string());
    }

    let drivers = driver_configs_from_payload(payload)?;
    let safe_state = payload.safe_state.clone().unwrap_or_default();
    let io_text = render_io_toml(drivers, safe_state);
    crate::config::validate_io_toml_text(&io_text)?;
    std::fs::write(&io_path, io_text).map_err(|err| {
        RuntimeError::ControlError(format!("failed to write io.toml: {err}").into())
    })?;
    Ok("✓ I/O config saved. Restart the runtime to apply.".to_string())
}

pub(super) fn render_io_toml(
    drivers: Vec<IoDriverConfig>,
    safe_state: Vec<IoSafeStateEntry>,
) -> String {
    let template = IoConfigTemplate {
        drivers: drivers
            .into_iter()
            .map(|driver| IoDriverTemplate {
                name: driver.name.to_string(),
                params: driver.params,
            })
            .collect(),
        safe_state: safe_state
            .into_iter()
            .map(|entry| (entry.address, entry.value))
            .collect(),
    };
    crate::bundle_template::render_io_toml(&template)
}

pub(super) fn driver_configs_from_payload(
    payload: &IoConfigRequest,
) -> Result<Vec<IoDriverConfig>, RuntimeError> {
    if let Some(drivers) = payload.drivers.as_ref() {
        if drivers.is_empty() {
            return Err(RuntimeError::InvalidConfig(
                "io.drivers must contain at least one driver".into(),
            ));
        }
        return drivers
            .iter()
            .enumerate()
            .map(|(idx, driver)| {
                let name = driver.name.trim();
                if name.is_empty() {
                    return Err(RuntimeError::InvalidConfig(
                        format!("io.drivers[{idx}].name must not be empty").into(),
                    ));
                }
                let params_json = driver.params.clone().unwrap_or_else(|| json!({}));
                let params_toml = json_to_toml(&params_json);
                if !params_toml.is_table() {
                    return Err(RuntimeError::InvalidConfig(
                        format!("io.drivers[{idx}].params must be a table/object").into(),
                    ));
                }
                Ok(IoDriverConfig {
                    name: SmolStr::new(name),
                    params: params_toml,
                    enabled: true,
                })
            })
            .collect::<Result<Vec<_>, _>>();
    }

    let driver = payload
        .driver
        .as_deref()
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .ok_or_else(|| RuntimeError::InvalidConfig("driver is required".into()))?;
    let params_json = payload.params.clone().unwrap_or_else(|| json!({}));
    let params_toml = json_to_toml(&params_json);
    if !params_toml.is_table() {
        return Err(RuntimeError::InvalidConfig(
            "params must be a table/object".into(),
        ));
    }
    Ok(vec![IoDriverConfig {
        name: SmolStr::new(driver),
        params: params_toml,
        enabled: true,
    }])
}

fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        IoArea::Input => "I",
        IoArea::Output => "Q",
        IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
        IoSize::Bytes(_) => "B",
    };
    if address.wildcard {
        return format!("%{}*", area);
    }
    if matches!(address.size, IoSize::Bit) {
        format!("%{}{}{}.{}", area, size, address.byte, address.bit)
    } else {
        format!("%{}{}{}", area, size, address.byte)
    }
}

fn format_io_safe_state_value(value: &crate::value::Value) -> String {
    match value {
        crate::value::Value::Bool(value) => {
            if *value {
                "TRUE".to_string()
            } else {
                "FALSE".to_string()
            }
        }
        crate::value::Value::SInt(value) => value.to_string(),
        crate::value::Value::USInt(value) => value.to_string(),
        crate::value::Value::Int(value) => value.to_string(),
        crate::value::Value::UInt(value) => value.to_string(),
        crate::value::Value::DInt(value) => value.to_string(),
        crate::value::Value::UDInt(value) => value.to_string(),
        crate::value::Value::LInt(value) => value.to_string(),
        crate::value::Value::ULInt(value) => value.to_string(),
        crate::value::Value::Byte(value) => value.to_string(),
        crate::value::Value::Word(value) => value.to_string(),
        crate::value::Value::DWord(value) => value.to_string(),
        crate::value::Value::LWord(value) => value.to_string(),
        _ => format_value(value),
    }
}

pub(super) fn list_sources(bundle_root: &Path) -> Vec<String> {
    let sources_dir = bundle_root.join("src");
    let mut list = Vec::new();
    let Ok(entries) = std::fs::read_dir(&sources_dir) else {
        return list;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if !entry.file_type().is_ok_and(|file_type| file_type.is_file())
            || !path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extension.eq_ignore_ascii_case("st"))
        {
            continue;
        }
        if let Some(name) = path.file_name().and_then(|v| v.to_str()) {
            list.push(name.to_string());
        }
    }
    list.sort_by_key(|name| name.to_ascii_lowercase());
    list
}

pub(super) fn read_source_file(bundle_root: &Path, name: &str) -> Result<String, RuntimeError> {
    let sources_dir = bundle_root.join("src");
    let requested = sources_dir.join(name);
    let sources_dir = sources_dir
        .canonicalize()
        .map_err(|err| RuntimeError::InvalidConfig(format!("src dir unavailable: {err}").into()))?;
    let requested = requested
        .canonicalize()
        .map_err(|err| RuntimeError::InvalidConfig(format!("source not found: {err}").into()))?;
    if !requested.starts_with(&sources_dir) {
        return Err(RuntimeError::InvalidConfig("invalid source path".into()));
    }
    std::fs::read_to_string(&requested)
        .map_err(|err| RuntimeError::InvalidConfig(format!("failed to read source: {err}").into()))
}

pub(super) fn read_hmi_asset_file(project_root: &Path, name: &str) -> Result<String, RuntimeError> {
    let hmi_dir = project_root.join("hmi");
    let requested = hmi_dir.join(name);
    let hmi_dir = hmi_dir
        .canonicalize()
        .map_err(|err| RuntimeError::InvalidConfig(format!("hmi dir unavailable: {err}").into()))?;
    let requested = requested
        .canonicalize()
        .map_err(|err| RuntimeError::InvalidConfig(format!("hmi asset not found: {err}").into()))?;
    if !requested.starts_with(&hmi_dir) {
        return Err(RuntimeError::InvalidConfig("invalid hmi asset path".into()));
    }
    if requested.extension().and_then(|value| value.to_str()) != Some("svg") {
        return Err(RuntimeError::InvalidConfig(
            "unsupported hmi asset type (only .svg is allowed)".into(),
        ));
    }
    std::fs::read_to_string(&requested).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to read hmi asset '{}': {err}", name).into())
    })
}

pub(super) fn apply_setup(
    bundle_root: &Option<PathBuf>,
    payload: SetupApplyRequest,
) -> Result<String, RuntimeError> {
    let defaults = setup_defaults(bundle_root);
    let bundle_path = payload
        .project_path
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.project_path.clone());
    let bundle_root = PathBuf::from(bundle_path);
    std::fs::create_dir_all(&bundle_root).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to create project folder: {err}").into())
    })?;

    let resource_name = payload
        .resource_name
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.resource_name);
    let cycle_ms = payload.cycle_ms.unwrap_or(defaults.cycle_ms);
    let mut driver = payload
        .driver
        .filter(|value| !value.trim().is_empty())
        .unwrap_or(defaults.driver);
    if driver == "auto" {
        driver = detect_default_driver();
    }

    let use_system_io = payload.use_system_io.unwrap_or(defaults.use_system_io);
    let write_system_io = payload.write_system_io.unwrap_or(defaults.write_system_io);
    let overwrite_system_io = payload.overwrite_system_io.unwrap_or(false);

    let runtime_path = bundle_root.join("runtime.toml");
    let runtime_text =
        crate::bundle_template::render_runtime_toml(&SmolStr::new(resource_name), cycle_ms);
    crate::config::validate_runtime_toml_text(&runtime_text)?;
    std::fs::write(&runtime_path, runtime_text).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to write runtime.toml: {err}").into())
    })?;

    let io_path = bundle_root.join("io.toml");
    if use_system_io {
        if io_path.exists() {
            std::fs::remove_file(&io_path).map_err(|err| {
                RuntimeError::InvalidConfig(format!("failed to remove io.toml: {err}").into())
            })?;
        }
    } else {
        let template =
            crate::bundle_template::build_io_config_auto(driver.as_str()).map_err(|err| {
                RuntimeError::InvalidConfig(format!("io template error: {err}").into())
            })?;
        let io_text = crate::bundle_template::render_io_toml(&template);
        crate::config::validate_io_toml_text(&io_text)?;
        std::fs::write(&io_path, io_text).map_err(|err| {
            RuntimeError::InvalidConfig(format!("failed to write io.toml: {err}").into())
        })?;
    }

    if write_system_io {
        let options = SetupOptions {
            driver: Some(SmolStr::new(driver)),
            backend: None,
            force: overwrite_system_io,
            path: None,
        };
        crate::setup::run_setup(options)?;
    }

    Ok("✓ Setup applied. Restart the runtime to load the new configuration.".to_string())
}

#[cfg(test)]
#[path = "setup_support/contract_tests.rs"]
mod contract_tests;
