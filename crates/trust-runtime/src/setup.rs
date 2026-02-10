//! System setup helpers (writes system IO configuration).

use std::fs;
use std::path::{Path, PathBuf};

use smol_str::SmolStr;

use crate::bundle_template::{IoConfigTemplate, IoDriverTemplate};
use crate::config::{system_io_config_path, IoConfig};
use crate::error::RuntimeError;

/// Options for `trust-runtime setup`.
#[derive(Debug, Clone)]
pub struct SetupOptions {
    /// Optional driver override (default is auto-detect).
    pub driver: Option<SmolStr>,
    /// Optional backend override (e.g., sysfs).
    pub backend: Option<SmolStr>,
    /// Overwrite existing system config.
    pub force: bool,
    /// Optional config output path override.
    pub path: Option<PathBuf>,
}

/// Run system setup and write the system IO config.
pub fn run_setup(options: SetupOptions) -> Result<PathBuf, RuntimeError> {
    let path = options.path.unwrap_or_else(system_io_config_path);
    if path.exists() && !options.force {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "system io config already exists at {} (use --force to overwrite)",
                path.display()
            )
            .into(),
        ));
    }

    let (driver, params) = match options.driver.as_deref() {
        Some(driver) => (
            driver.to_string(),
            build_driver_params(driver, options.backend)?,
        ),
        None => detect_default_driver(options.backend)?,
    };

    let io_config = IoConfig {
        drivers: vec![crate::config::IoDriverConfig {
            name: SmolStr::new(driver),
            params,
        }],
        safe_state: crate::io::IoSafeState::default(),
    };

    write_system_io_config(&path, &io_config)?;
    Ok(path)
}

/// Best-effort Raspberry Pi detection (returns false on error).
pub fn is_raspberry_pi_hint() -> bool {
    is_raspberry_pi().unwrap_or(false)
}

fn detect_default_driver(backend: Option<SmolStr>) -> Result<(String, toml::Value), RuntimeError> {
    if is_raspberry_pi()? {
        let params = build_gpio_params(backend)?;
        return Ok(("gpio".to_string(), params));
    }
    Ok((
        "loopback".to_string(),
        toml::Value::Table(toml::map::Map::new()),
    ))
}

fn build_driver_params(
    driver: &str,
    backend: Option<SmolStr>,
) -> Result<toml::Value, RuntimeError> {
    if driver.eq_ignore_ascii_case("gpio") {
        return build_gpio_params(backend);
    }
    if driver.eq_ignore_ascii_case("modbus-tcp") {
        return Ok(default_modbus_params());
    }
    if driver.eq_ignore_ascii_case("mqtt") {
        return Ok(default_mqtt_params());
    }
    if driver.eq_ignore_ascii_case("simulated") || driver.eq_ignore_ascii_case("loopback") {
        return Ok(toml::Value::Table(toml::map::Map::new()));
    }
    Err(RuntimeError::InvalidConfig(
        format!(
            "invalid I/O driver '{driver}'. Expected: loopback, gpio, simulated, modbus-tcp, or mqtt."
        )
        .into(),
    ))
}

fn build_gpio_params(backend: Option<SmolStr>) -> Result<toml::Value, RuntimeError> {
    let mut params = toml::map::Map::new();
    let backend = backend.unwrap_or_else(|| SmolStr::new("sysfs"));
    params.insert("backend".into(), toml::Value::String(backend.to_string()));
    params.insert("inputs".into(), toml::Value::Array(Vec::new()));
    params.insert("outputs".into(), toml::Value::Array(Vec::new()));
    Ok(toml::Value::Table(params))
}

fn default_modbus_params() -> toml::Value {
    let mut params = toml::map::Map::new();
    params.insert(
        "address".into(),
        toml::Value::String("127.0.0.1:502".to_string()),
    );
    params.insert("unit_id".into(), toml::Value::Integer(1));
    params.insert("input_start".into(), toml::Value::Integer(0));
    params.insert("output_start".into(), toml::Value::Integer(0));
    params.insert("timeout_ms".into(), toml::Value::Integer(500));
    params.insert("on_error".into(), toml::Value::String("fault".to_string()));
    toml::Value::Table(params)
}

fn default_mqtt_params() -> toml::Value {
    let mut params = toml::map::Map::new();
    params.insert(
        "broker".into(),
        toml::Value::String("127.0.0.1:1883".to_string()),
    );
    params.insert(
        "topic_in".into(),
        toml::Value::String("trust/io/in".to_string()),
    );
    params.insert(
        "topic_out".into(),
        toml::Value::String("trust/io/out".to_string()),
    );
    params.insert("reconnect_ms".into(), toml::Value::Integer(500));
    params.insert("keep_alive_s".into(), toml::Value::Integer(5));
    params.insert("allow_insecure_remote".into(), toml::Value::Boolean(false));
    toml::Value::Table(params)
}

fn write_system_io_config(path: &Path, config: &IoConfig) -> Result<(), RuntimeError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|err| {
            RuntimeError::InvalidConfig(
                format!("failed to create {}: {err}", parent.display()).into(),
            )
        })?;
    }
    let text = render_io_config(config)?;
    fs::write(path, text).map_err(|err| {
        RuntimeError::InvalidConfig(format!("failed to write {}: {err}", path.display()).into())
    })?;
    Ok(())
}

fn render_io_config(config: &IoConfig) -> Result<String, RuntimeError> {
    let template = IoConfigTemplate {
        drivers: config
            .drivers
            .iter()
            .map(|driver| IoDriverTemplate {
                name: driver.name.to_string(),
                params: driver.params.clone(),
            })
            .collect(),
        safe_state: Vec::new(),
    };
    Ok(crate::bundle_template::render_io_toml(&template))
}

fn is_raspberry_pi() -> Result<bool, RuntimeError> {
    let candidates = [
        "/proc/device-tree/model",
        "/sys/firmware/devicetree/base/model",
    ];
    for path in candidates {
        if let Ok(text) = fs::read_to_string(path) {
            if text.to_ascii_lowercase().contains("raspberry pi") {
                return Ok(true);
            }
        }
    }
    Ok(false)
}
