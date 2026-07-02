use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;

use crate::config::IoDriverConfig;
use crate::error::RuntimeError;
use crate::io::IoDriverRegistry;

use super::contract::COMM_SCHEMA_VERSION;
use super::schema::{
    driver_to_protocol, normalize_protocol, protocol_to_driver, supports_runtime_file_protocol,
};
use super::{ControlResponse, ControlState};

mod io_file;
mod runtime_file;
mod validation;

use validation::{field_from_error, validate_schema_fields};

#[derive(Debug, Deserialize)]
pub(super) struct CommApplyRequest {
    protocol: String,
    #[serde(default)]
    pub(super) action: CommApplyAction,
    pub(super) instance_id: Option<String>,
    pub(super) instance_name: Option<String>,
    #[serde(default)]
    pub(super) params: serde_json::Value,
    #[serde(default)]
    pub(super) dry_run: bool,
    pub(super) credential_channel: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(super) enum CommApplyAction {
    Add,
    Edit,
    #[default]
    Upsert,
    Remove,
    Disable,
    Validate,
}

#[derive(Debug, Serialize)]
pub(super) struct CommApplyResponse {
    pub(super) schema_version: u32,
    pub(super) protocol: String,
    pub(super) driver: String,
    pub(super) action: CommApplyAction,
    pub(super) applied: bool,
    pub(super) lifecycle_effect: &'static str,
    pub(super) message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) instance_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) field_errors: Vec<CommFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(super) struct CommFieldError {
    pub(super) field: String,
    pub(super) message: String,
}

struct LoadedIoConfig {
    path: PathBuf,
    drivers: Vec<IoDriverConfig>,
    safe_state: Vec<(String, String)>,
}

pub(super) fn handle_comm_apply(
    id: u64,
    params: Option<serde_json::Value>,
    state: &ControlState,
) -> ControlResponse {
    let params = match params {
        Some(params) => params,
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let request: CommApplyRequest = match serde_json::from_value(params) {
        Ok(value) => value,
        Err(error) => {
            return ControlResponse::error(id, format!("invalid comm.apply payload: {error}"))
        }
    };
    let response = apply_request(state, request);
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("Communication apply serialization failed: {error}"),
        ),
    }
}

pub(super) fn apply_project_value(
    project_root: &Path,
    params: serde_json::Value,
) -> Result<serde_json::Value, String> {
    let mut request: CommApplyRequest = serde_json::from_value(params)
        .map_err(|error| format!("invalid comm.apply payload: {error}"))?;
    if request.credential_channel.is_none() {
        request.credential_channel = Some("trusted_same_host".to_string());
    }
    if matches!(
        request.action,
        CommApplyAction::Remove | CommApplyAction::Disable
    ) && request.instance_id.is_none()
    {
        request.instance_id = resolve_project_instance_id(project_root, &request);
    }
    let response = apply_request_with_project_root(Some(project_root), request);
    serde_json::to_value(response)
        .map_err(|error| format!("Communication apply serialization failed: {error}"))
}

fn apply_request(state: &ControlState, request: CommApplyRequest) -> CommApplyResponse {
    apply_request_with_project_root(state.project_root.as_deref(), request)
}

fn apply_request_with_project_root(
    project_root: Option<&Path>,
    request: CommApplyRequest,
) -> CommApplyResponse {
    let protocol = normalize_protocol(request.protocol.as_str());
    let Some(driver) = protocol_to_driver(protocol.as_str()) else {
        if supports_runtime_file_protocol(protocol.as_str()) {
            return runtime_file::apply_runtime_protocol(project_root, protocol, request);
        }
        if matches!(protocol.as_str(), "ads" | "ads_server") {
            return runtime_file::apply_runtime_protocol(project_root, protocol, request);
        }
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "protocol",
                "Unsupported Communication protocol.",
            )],
            None,
            None,
        );
    };
    let Some(project_root) = project_root else {
        return blocked_response(
            protocol,
            driver.to_string(),
            request.action,
            vec![field_error(
                "project_root",
                "No runtime project root is available for io.toml setup.",
            )],
            None,
            None,
        );
    };

    let mut loaded = match load_io_config(project_root) {
        Ok(value) => value,
        Err(error) => {
            return blocked_response(
                protocol,
                driver.to_string(),
                request.action,
                vec![field_error("_", error.to_string())],
                Some(project_root.join("io.toml").display().to_string()),
                None,
            )
        }
    };

    let mut field_errors = Vec::new();
    if request
        .instance_name
        .as_deref()
        .is_some_and(|value| value.trim().is_empty())
    {
        field_errors.push(field_error(
            "instance_name",
            "Instance name must not be empty when provided.",
        ));
    }
    if secret_values_present(&request.params)
        && request.credential_channel.as_deref() != Some("trusted_same_host")
    {
        field_errors.push(field_error(
            "password",
            "Secret fields cannot be sent over an untrusted runtime control channel.",
        ));
    }

    match request.action {
        CommApplyAction::Remove => {
            if let Err(error) = remove_instance(&mut loaded.drivers, &request) {
                field_errors.push(error);
            }
        }
        CommApplyAction::Disable => {
            if let Err(error) = disable_instance(&mut loaded.drivers, &request) {
                field_errors.push(error);
            }
        }
        CommApplyAction::Validate
        | CommApplyAction::Add
        | CommApplyAction::Edit
        | CommApplyAction::Upsert => match build_driver_config(driver, &request.params) {
            Ok(driver_config) => {
                field_errors.extend(validate_driver_config(&protocol, &driver_config));
                if field_errors.is_empty() && request.action != CommApplyAction::Validate {
                    if let Err(error) = upsert_instance(
                        &mut loaded.drivers,
                        driver_config,
                        protocol.as_str(),
                        &request,
                    ) {
                        field_errors.push(error);
                    }
                }
            }
            Err(errors) => field_errors.extend(errors),
        },
    }

    if !field_errors.is_empty() {
        return blocked_response(
            protocol,
            driver.to_string(),
            request.action,
            field_errors,
            Some(loaded.path.display().to_string()),
            request.instance_id,
        );
    }

    if loaded.drivers.is_empty()
        && matches!(
            request.action,
            CommApplyAction::Remove | CommApplyAction::Disable
        )
    {
        return remove_io_config_response(protocol, driver.to_string(), request, loaded);
    }

    let io_text = match io_file::render_io_toml(&loaded.path, &loaded.drivers, &loaded.safe_state) {
        Ok(text) => text,
        Err(error) => {
            return blocked_response(
                protocol,
                driver.to_string(),
                request.action,
                vec![error],
                Some(loaded.path.display().to_string()),
                request.instance_id,
            )
        }
    };
    if let Err(error) = crate::config::validate_io_toml_text(&io_text) {
        return blocked_response(
            protocol,
            driver.to_string(),
            request.action,
            vec![field_error("_", error.to_string())],
            Some(loaded.path.display().to_string()),
            request.instance_id,
        );
    }

    if request.dry_run || request.action == CommApplyAction::Validate {
        return CommApplyResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            driver: driver.to_string(),
            action: request.action,
            applied: false,
            lifecycle_effect: "validate_only",
            message: "Configuration validated. No files were changed.".to_string(),
            config_path: Some(loaded.path.display().to_string()),
            instance_id: request.instance_id,
            field_errors: Vec::new(),
            snippet: None,
        };
    }

    if let Err(error) = std::fs::write(&loaded.path, &io_text) {
        return blocked_response(
            protocol,
            driver.to_string(),
            request.action,
            vec![field_error(
                "_",
                format!("failed to write {}: {error}", loaded.path.display()),
            )],
            Some(loaded.path.display().to_string()),
            request.instance_id,
        );
    }

    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver: driver.to_string(),
        action: request.action,
        applied: true,
        lifecycle_effect: "restart_required",
        message: io_apply_message(request.action).to_string(),
        config_path: Some(loaded.path.display().to_string()),
        instance_id: request.instance_id,
        field_errors: Vec::new(),
        snippet: None,
    }
}

fn io_apply_message(action: CommApplyAction) -> &'static str {
    match action {
        CommApplyAction::Disable => {
            "Endpoint disabled. Restart the runtime to apply it; the endpoint stays visible in Devices & Connections."
        }
        _ => "I/O configuration saved. Restart the runtime to apply it.",
    }
}

fn resolve_project_instance_id(project_root: &Path, request: &CommApplyRequest) -> Option<String> {
    let loaded = load_io_config(project_root).ok()?;
    let requested_protocol = normalize_protocol(request.protocol.as_str());
    let requested_params = normalized_request_params(&request.params);
    let mut matches = loaded
        .drivers
        .iter()
        .enumerate()
        .filter(|(_, driver)| {
            driver_to_protocol(driver.name.as_str()) == Some(requested_protocol.as_str())
        })
        .filter(|(_, driver)| params_match(requested_params.as_ref(), &driver.params))
        .map(|(index, _)| format!("{requested_protocol}:{index}"));
    let first = matches.next()?;
    if matches.next().is_none() {
        Some(first)
    } else {
        None
    }
}

fn normalized_request_params(params: &serde_json::Value) -> Option<toml::Value> {
    match params {
        serde_json::Value::Null => Some(toml::Value::Table(Default::default())),
        serde_json::Value::Object(_) => {
            let mut value = json_to_toml(params);
            strip_empty_optional_values(&mut value);
            Some(value)
        }
        _ => None,
    }
}

fn params_match(requested: Option<&toml::Value>, existing: &toml::Value) -> bool {
    let Some(requested) = requested else {
        return true;
    };
    let Some(requested) = requested.as_table() else {
        return false;
    };
    if requested.is_empty() {
        return true;
    }
    requested
        .iter()
        .all(|(key, value)| existing.get(key).is_some_and(|existing| existing == value))
}

fn remove_io_config_response(
    protocol: String,
    driver: String,
    request: CommApplyRequest,
    loaded: LoadedIoConfig,
) -> CommApplyResponse {
    if request.dry_run || request.action == CommApplyAction::Validate {
        return CommApplyResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            driver,
            action: request.action,
            applied: false,
            lifecycle_effect: "validate_only",
            message: "I/O configuration removal validated. No files were changed.".to_string(),
            config_path: Some(loaded.path.display().to_string()),
            instance_id: request.instance_id,
            field_errors: Vec::new(),
            snippet: None,
        };
    }

    if let Err(error) = remove_io_config_file(&loaded.path) {
        return blocked_response(
            protocol,
            driver,
            request.action,
            vec![field_error(
                "_",
                format!("failed to remove {}: {error}", loaded.path.display()),
            )],
            Some(loaded.path.display().to_string()),
            request.instance_id,
        );
    }

    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver,
        action: request.action,
        applied: true,
        lifecycle_effect: "restart_required",
        message: "I/O configuration removed. Restart the runtime to apply it.".to_string(),
        config_path: Some(loaded.path.display().to_string()),
        instance_id: request.instance_id,
        field_errors: Vec::new(),
        snippet: None,
    }
}

fn remove_io_config_file(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}

fn load_io_config(project_root: &Path) -> Result<LoadedIoConfig, RuntimeError> {
    let path = project_root.join("io.toml");
    if path.is_file() {
        return load_editable_io_config(&path);
    }
    Ok(LoadedIoConfig {
        path,
        drivers: Vec::new(),
        safe_state: vec![("%QX0.0".to_string(), "FALSE".to_string())],
    })
}

fn load_editable_io_config(path: &Path) -> Result<LoadedIoConfig, RuntimeError> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        RuntimeError::IoDriver(SmolStr::new(format!(
            "failed to read {}: {error}",
            path.display()
        )))
    })?;
    let value = text.parse::<toml::Table>().map_err(|error| {
        RuntimeError::IoDriver(SmolStr::new(format!("invalid io.toml: {error}")))
    })?;
    let Some(io) = value.get("io").and_then(toml::Value::as_table) else {
        return Ok(LoadedIoConfig {
            path: path.to_path_buf(),
            drivers: Vec::new(),
            safe_state: vec![("%QX0.0".to_string(), "FALSE".to_string())],
        });
    };
    Ok(LoadedIoConfig {
        path: path.to_path_buf(),
        drivers: io_drivers_from_toml(io)?,
        safe_state: safe_state_entries_from_toml(io.get("safe_state"))?,
    })
}

fn io_drivers_from_toml(
    io: &toml::map::Map<String, toml::Value>,
) -> Result<Vec<IoDriverConfig>, RuntimeError> {
    let mut drivers = Vec::new();
    if let Some(driver) = io
        .get("driver")
        .and_then(toml::Value::as_str)
        .map(str::trim)
        .filter(|driver| !driver.is_empty())
    {
        let params = io
            .get("params")
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        if !params.is_table() {
            return Err(RuntimeError::IoDriver(SmolStr::new(
                "invalid io.toml: io.params must be a table",
            )));
        }
        drivers.push(IoDriverConfig {
            name: SmolStr::new(driver),
            params,
            enabled: true,
        });
    }
    let Some(explicit_drivers) = io.get("drivers") else {
        return Ok(drivers);
    };
    let explicit_drivers = explicit_drivers.as_array().ok_or_else(|| {
        RuntimeError::IoDriver(SmolStr::new(
            "invalid io.toml: io.drivers must be an array of tables",
        ))
    })?;
    for (index, driver) in explicit_drivers.iter().enumerate() {
        let table = driver.as_table().ok_or_else(|| {
            RuntimeError::IoDriver(SmolStr::new(format!(
                "invalid io.toml: io.drivers[{index}] must be a table"
            )))
        })?;
        let name = table
            .get("name")
            .or_else(|| table.get("driver"))
            .and_then(toml::Value::as_str)
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .ok_or_else(|| {
                RuntimeError::IoDriver(SmolStr::new(format!(
                    "invalid io.toml: io.drivers[{index}].name must not be empty"
                )))
            })?;
        let params = table
            .get("params")
            .cloned()
            .unwrap_or_else(|| toml::Value::Table(toml::map::Map::new()));
        if !params.is_table() {
            return Err(RuntimeError::IoDriver(SmolStr::new(format!(
                "invalid io.toml: io.drivers[{index}].params must be a table"
            ))));
        }
        drivers.push(IoDriverConfig {
            name: SmolStr::new(name),
            params,
            enabled: table
                .get("enabled")
                .and_then(toml::Value::as_bool)
                .unwrap_or(true),
        });
    }
    Ok(drivers)
}

fn safe_state_entries_from_toml(
    value: Option<&toml::Value>,
) -> Result<Vec<(String, String)>, RuntimeError> {
    let Some(value) = value else {
        return Ok(vec![("%QX0.0".to_string(), "FALSE".to_string())]);
    };
    let entries = value.as_array().ok_or_else(|| {
        RuntimeError::IoDriver(SmolStr::new(
            "invalid io.toml: io.safe_state must be an array",
        ))
    })?;
    let mut safe_state = Vec::new();
    for entry in entries {
        let table = entry.as_table().ok_or_else(|| {
            RuntimeError::IoDriver(SmolStr::new(
                "invalid io.toml: each io.safe_state entry must be a table",
            ))
        })?;
        let address = table
            .get("address")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                RuntimeError::IoDriver(SmolStr::new(
                    "invalid io.toml: io.safe_state entry missing address",
                ))
            })?;
        let value = table
            .get("value")
            .and_then(toml::Value::as_str)
            .ok_or_else(|| {
                RuntimeError::IoDriver(SmolStr::new(
                    "invalid io.toml: io.safe_state entry missing value",
                ))
            })?;
        safe_state.push((address.to_string(), value.to_string()));
    }
    Ok(safe_state)
}

fn build_driver_config(
    driver: &str,
    params: &serde_json::Value,
) -> Result<IoDriverConfig, Vec<CommFieldError>> {
    let params = match params {
        serde_json::Value::Null => serde_json::Value::Object(Default::default()),
        serde_json::Value::Object(_) => params.clone(),
        _ => return Err(vec![field_error("params", "Parameters must be an object.")]),
    };
    let mut params_toml = json_to_toml(&params);
    strip_empty_optional_values(&mut params_toml);
    if !params_toml.is_table() {
        return Err(vec![field_error(
            "params",
            "Parameters must be a table/object.",
        )]);
    }
    Ok(IoDriverConfig {
        name: SmolStr::new(driver),
        params: params_toml,
        enabled: true,
    })
}

fn validate_driver_config(protocol: &str, driver: &IoDriverConfig) -> Vec<CommFieldError> {
    let mut errors = validate_schema_fields(protocol, &driver.params);
    if !errors.is_empty() {
        return errors;
    }
    if let Err(error) =
        IoDriverRegistry::default_registry().validate(driver.name.as_str(), &driver.params)
    {
        errors.push(field_error(
            field_from_error(&error.to_string()),
            error.to_string(),
        ));
    }
    errors
}

fn upsert_instance(
    drivers: &mut Vec<IoDriverConfig>,
    driver_config: IoDriverConfig,
    protocol: &str,
    request: &CommApplyRequest,
) -> Result<(), CommFieldError> {
    if matches!(request.action, CommApplyAction::Add) {
        drivers.push(driver_config);
        return Ok(());
    }
    if matches!(request.action, CommApplyAction::Edit) && request.instance_id.is_none() {
        return Err(field_error(
            "instance_id",
            "Choose the configured instance to update.",
        ));
    }
    if let Some((instance_protocol, index)) = parse_instance_id(request.instance_id.as_deref()) {
        if instance_protocol != protocol {
            return Err(field_error(
                "instance_id",
                "Configured instance belongs to a different protocol.",
            ));
        }
        if let Some(slot) = drivers.get_mut(index) {
            if driver_to_protocol(slot.name.as_str()) != Some(protocol) {
                return Err(field_error(
                    "instance_id",
                    "Configured instance belongs to a different protocol.",
                ));
            }
            *slot = driver_config;
            return Ok(());
        }
        return Err(field_error(
            "instance_id",
            "Configured instance was not found.",
        ));
    }
    if let Some((_, slot)) = drivers
        .iter_mut()
        .enumerate()
        .find(|(_, driver)| same_driver(driver.name.as_str(), driver_config.name.as_str()))
    {
        *slot = driver_config;
        return Ok(());
    }
    drivers.push(driver_config);
    Ok(())
}

fn remove_instance(
    drivers: &mut Vec<IoDriverConfig>,
    request: &CommApplyRequest,
) -> Result<(), CommFieldError> {
    let requested_protocol = normalize_protocol(request.protocol.as_str());
    let Some((instance_protocol, index)) = parse_instance_id(request.instance_id.as_deref()) else {
        return Err(field_error(
            "instance_id",
            "Choose the configured instance to remove.",
        ));
    };
    if instance_protocol != requested_protocol {
        return Err(field_error(
            "instance_id",
            "Configured instance belongs to a different protocol.",
        ));
    }
    if index >= drivers.len() {
        return Err(field_error(
            "instance_id",
            "Configured instance was not found.",
        ));
    }
    if driver_to_protocol(drivers[index].name.as_str()) != Some(requested_protocol.as_str()) {
        return Err(field_error(
            "instance_id",
            "Configured instance belongs to a different protocol.",
        ));
    }
    drivers.remove(index);
    Ok(())
}

fn disable_instance(
    drivers: &mut [IoDriverConfig],
    request: &CommApplyRequest,
) -> Result<(), CommFieldError> {
    let requested_protocol = normalize_protocol(request.protocol.as_str());
    let Some((instance_protocol, index)) = parse_instance_id(request.instance_id.as_deref()) else {
        return Err(field_error(
            "instance_id",
            "Choose the configured instance to disable.",
        ));
    };
    if instance_protocol != requested_protocol {
        return Err(field_error(
            "instance_id",
            "Configured instance belongs to a different protocol.",
        ));
    }
    let Some(driver) = drivers.get_mut(index) else {
        return Err(field_error(
            "instance_id",
            "Configured instance was not found.",
        ));
    };
    if driver_to_protocol(driver.name.as_str()) != Some(requested_protocol.as_str()) {
        return Err(field_error(
            "instance_id",
            "Configured instance belongs to a different protocol.",
        ));
    }
    driver.enabled = false;
    Ok(())
}

fn parse_instance_id(instance_id: Option<&str>) -> Option<(String, usize)> {
    let value = instance_id?;
    let (protocol, index) = value.rsplit_once(':')?;
    Some((normalize_protocol(protocol), index.parse::<usize>().ok()?))
}

fn same_driver(left: &str, right: &str) -> bool {
    driver_to_protocol(left) == driver_to_protocol(right)
}

pub(super) fn blocked_response(
    protocol: String,
    driver: String,
    action: CommApplyAction,
    field_errors: Vec<CommFieldError>,
    config_path: Option<String>,
    instance_id: Option<String>,
) -> CommApplyResponse {
    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver,
        action,
        applied: false,
        lifecycle_effect: "blocked",
        message: "Configuration was not applied. Fix the highlighted fields and try again."
            .to_string(),
        config_path,
        instance_id,
        field_errors,
        snippet: None,
    }
}

pub(super) fn field_error(field: impl Into<String>, message: impl Into<String>) -> CommFieldError {
    CommFieldError {
        field: field.into(),
        message: message.into(),
    }
}

pub(super) fn json_to_toml(value: &serde_json::Value) -> toml::Value {
    match value {
        serde_json::Value::Null => toml::Value::String(String::new()),
        serde_json::Value::Bool(value) => toml::Value::Boolean(*value),
        serde_json::Value::Number(value) => {
            if let Some(i) = value.as_i64() {
                toml::Value::Integer(i)
            } else if let Some(u) = value.as_u64() {
                toml::Value::Integer(u.min(i64::MAX as u64) as i64)
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

pub(super) fn strip_empty_optional_values(value: &mut toml::Value) {
    let Some(table) = value.as_table_mut() else {
        return;
    };
    let optional_empty = [
        "client_id",
        "username",
        "password",
        "tls_ca_path",
        "tls_client_cert_path",
        "tls_client_key_path",
        "auth_token",
        "host_group",
        "note",
    ];
    for field in optional_empty {
        let remove = table
            .get(field)
            .and_then(toml::Value::as_str)
            .is_some_and(|value| value.trim().is_empty());
        if remove {
            table.remove(field);
        }
    }
}

pub(super) fn secret_values_present(params: &serde_json::Value) -> bool {
    match params {
        serde_json::Value::Object(values) => values.iter().any(|(key, value)| {
            is_secret_key(key) && value.as_str().is_some_and(|value| !value.trim().is_empty())
                || secret_values_present(value)
        }),
        serde_json::Value::Array(values) => values.iter().any(secret_values_present),
        _ => false,
    }
}

fn is_secret_key(key: &str) -> bool {
    matches!(
        key.trim().to_ascii_lowercase().as_str(),
        "password" | "auth_token" | "token" | "secret" | "client_secret"
    )
}

pub(super) fn sanitized_apply_audit_details(params: &serde_json::Value) -> serde_json::Value {
    let protocol = params
        .get("protocol")
        .and_then(serde_json::Value::as_str)
        .map(normalize_protocol)
        .unwrap_or_default();
    json!({
        "protocol": protocol,
        "action": params.get("action").and_then(serde_json::Value::as_str).unwrap_or("upsert"),
        "instance_id": params.get("instance_id").and_then(serde_json::Value::as_str),
        "instance_name": params.get("instance_name").and_then(serde_json::Value::as_str),
        "dry_run": params.get("dry_run").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "secret_fields_present": secret_values_present(params.get("params").unwrap_or(&serde_json::Value::Null)),
    })
}
