use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use smol_str::SmolStr;

use crate::bundle_template::{IoConfigTemplate, IoDriverTemplate};
use crate::config::{IoConfig, IoDriverConfig};
use crate::error::RuntimeError;
use crate::io::{IoAddress, IoDriverRegistry, IoSafeState, IoSize};
use crate::value::Value;

use super::contract::COMM_SCHEMA_VERSION;
use super::schema::{
    driver_to_protocol, normalize_protocol, protocol_to_driver, supports_snippet_fallback,
};
use super::{ControlResponse, ControlState};

mod validation;

use validation::{field_from_error, validate_schema_fields, validate_snippet_fields};

#[derive(Debug, Deserialize)]
struct CommApplyRequest {
    protocol: String,
    #[serde(default)]
    action: CommApplyAction,
    instance_id: Option<String>,
    instance_name: Option<String>,
    #[serde(default)]
    params: serde_json::Value,
    #[serde(default)]
    dry_run: bool,
    credential_channel: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
enum CommApplyAction {
    Add,
    Edit,
    #[default]
    Upsert,
    Remove,
    Disable,
    Validate,
}

#[derive(Debug, Serialize)]
struct CommApplyResponse {
    schema_version: u32,
    protocol: String,
    driver: String,
    action: CommApplyAction,
    applied: bool,
    lifecycle_effect: &'static str,
    message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    config_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    instance_id: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    field_errors: Vec<CommFieldError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    snippet: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
struct CommFieldError {
    field: String,
    message: String,
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

fn apply_request(state: &ControlState, request: CommApplyRequest) -> CommApplyResponse {
    let protocol = normalize_protocol(request.protocol.as_str());
    let Some(driver) = protocol_to_driver(protocol.as_str()) else {
        if supports_snippet_fallback(protocol.as_str()) {
            return snippet_response(protocol, request);
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
    let Some(project_root) = state.project_root.as_ref() else {
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
        CommApplyAction::Remove | CommApplyAction::Disable => {
            if let Err(error) = remove_instance(&mut loaded.drivers, &request) {
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

    let io_text = render_io_toml(&loaded.drivers, &loaded.safe_state);
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
            snippet: Some(io_text),
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
        message: "I/O configuration saved. Restart the runtime to apply it.".to_string(),
        config_path: Some(loaded.path.display().to_string()),
        instance_id: request.instance_id,
        field_errors: Vec::new(),
        snippet: None,
    }
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

fn snippet_response(protocol: String, request: CommApplyRequest) -> CommApplyResponse {
    let mut field_errors = Vec::new();
    if secret_values_present(&request.params)
        && request.credential_channel.as_deref() != Some("trusted_same_host")
    {
        field_errors.push(field_error(
            "password",
            "Secret fields cannot be sent over an untrusted runtime control channel.",
        ));
    }
    let params = match &request.params {
        serde_json::Value::Null => serde_json::Value::Object(Default::default()),
        serde_json::Value::Object(_) => request.params.clone(),
        _ => {
            field_errors.push(field_error("params", "Parameters must be an object."));
            serde_json::Value::Object(Default::default())
        }
    };
    let mut params_toml = json_to_toml(&params);
    strip_empty_optional_values(&mut params_toml);
    if let Some(table) = params_toml.as_table() {
        field_errors.extend(validate_snippet_fields(protocol.as_str(), table));
    } else {
        field_errors.push(field_error("params", "Parameters must be a table/object."));
    }
    if !field_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            field_errors,
            Some("runtime.toml".to_string()),
            None,
        );
    }
    let snippet = match params_toml.as_table() {
        Some(table) => render_runtime_snippet(protocol.as_str(), table),
        None => String::new(),
    };
    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver: String::new(),
        action: request.action,
        applied: false,
        lifecycle_effect: "deploy_required",
        message: "Configuration validated. Paste this snippet into runtime.toml, then restart or deploy the runtime to apply it.".to_string(),
        config_path: Some("runtime.toml".to_string()),
        instance_id: None,
        field_errors: Vec::new(),
        snippet: Some(snippet),
    }
}

fn load_io_config(project_root: &Path) -> Result<LoadedIoConfig, RuntimeError> {
    let path = project_root.join("io.toml");
    if path.is_file() {
        let config = IoConfig::load(&path)?;
        return Ok(LoadedIoConfig {
            path,
            drivers: config.drivers,
            safe_state: format_safe_state(&config.safe_state),
        });
    }
    Ok(LoadedIoConfig {
        path,
        drivers: Vec::new(),
        safe_state: vec![("%QX0.0".to_string(), "FALSE".to_string())],
    })
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

fn render_runtime_snippet(protocol: &str, table: &toml::map::Map<String, toml::Value>) -> String {
    let mut root = toml::map::Map::new();
    let mut runtime = toml::map::Map::new();
    match protocol {
        "runtime_cloud" => {
            let mut cloud = toml::map::Map::new();
            if let Some(profile) = table.get("profile") {
                cloud.insert("profile".into(), profile.clone());
            }
            let mut wan = toml::map::Map::new();
            if let Some(rules) = table.get("wan_allow_write") {
                wan.insert("allow_write".into(), rules.clone());
            }
            if !wan.is_empty() {
                cloud.insert("wan".into(), toml::Value::Table(wan));
            }
            let mut links = toml::map::Map::new();
            if let Some(transports) = table.get("link_transports") {
                links.insert("transports".into(), transports.clone());
            }
            if !links.is_empty() {
                cloud.insert("links".into(), toml::Value::Table(links));
            }
            runtime.insert("cloud".into(), toml::Value::Table(cloud));
        }
        "realtime_t0" => {
            runtime.insert(
                "realtime".into(),
                toml::Value::Table(filtered_table(table, &[])),
            );
        }
        "opcua" => {
            let mut section = filtered_table(table, &[]);
            if section
                .get("password")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                section.insert(
                    "password".into(),
                    toml::Value::String("<set on runtime host>".into()),
                );
            }
            runtime.insert("opcua".into(), toml::Value::Table(section));
        }
        "mesh" => {
            let mut section = filtered_table(table, &[]);
            if section
                .get("auth_token")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty())
            {
                section.insert(
                    "auth_token".into(),
                    toml::Value::String("<set on runtime host>".into()),
                );
            }
            runtime.insert("mesh".into(), toml::Value::Table(section));
        }
        other => {
            runtime.insert(other.into(), toml::Value::Table(filtered_table(table, &[])));
        }
    }
    root.insert("runtime".into(), toml::Value::Table(runtime));
    toml::to_string_pretty(&toml::Value::Table(root)).unwrap_or_default()
}

fn filtered_table(
    table: &toml::map::Map<String, toml::Value>,
    skip: &[&str],
) -> toml::map::Map<String, toml::Value> {
    table
        .iter()
        .filter(|(key, value)| {
            !skip.contains(&key.as_str())
                && !matches!(value, toml::Value::String(text) if text.trim().is_empty())
        })
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
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

fn parse_instance_id(instance_id: Option<&str>) -> Option<(String, usize)> {
    let value = instance_id?;
    let (protocol, index) = value.rsplit_once(':')?;
    Some((normalize_protocol(protocol), index.parse::<usize>().ok()?))
}

fn same_driver(left: &str, right: &str) -> bool {
    driver_to_protocol(left) == driver_to_protocol(right)
}

fn render_io_toml(drivers: &[IoDriverConfig], safe_state: &[(String, String)]) -> String {
    let template = IoConfigTemplate {
        drivers: drivers
            .iter()
            .map(|driver| IoDriverTemplate {
                name: driver.name.to_string(),
                params: driver.params.clone(),
            })
            .collect(),
        safe_state: safe_state.to_vec(),
    };
    crate::bundle_template::render_io_toml(&template)
}

fn format_safe_state(safe_state: &IoSafeState) -> Vec<(String, String)> {
    safe_state
        .outputs
        .iter()
        .map(|(address, value)| (format_io_address(address), format_io_value(value)))
        .collect()
}

fn format_io_address(address: &IoAddress) -> String {
    let area = match address.area {
        crate::memory::IoArea::Input => "I",
        crate::memory::IoArea::Output => "Q",
        crate::memory::IoArea::Memory => "M",
    };
    let size = match address.size {
        IoSize::Bit => "X",
        IoSize::Byte => "B",
        IoSize::Word => "W",
        IoSize::DWord => "D",
        IoSize::LWord => "L",
    };
    if address.wildcard {
        return format!("%{area}{size}*");
    }
    if matches!(address.size, IoSize::Bit) {
        format!("%{area}{size}{}.{}", address.byte, address.bit)
    } else {
        format!("%{area}{size}{}", address.byte)
    }
}

fn format_io_value(value: &Value) -> String {
    match value {
        Value::Bool(value) => value.to_string().to_ascii_uppercase(),
        Value::SInt(value) => value.to_string(),
        Value::USInt(value) => value.to_string(),
        Value::Int(value) => value.to_string(),
        Value::UInt(value) => value.to_string(),
        Value::DInt(value) => value.to_string(),
        Value::UDInt(value) => value.to_string(),
        Value::LInt(value) => value.to_string(),
        Value::ULInt(value) => value.to_string(),
        Value::Byte(value) => value.to_string(),
        Value::Word(value) => value.to_string(),
        Value::DWord(value) => value.to_string(),
        Value::LWord(value) => value.to_string(),
        other => format!("{other:?}"),
    }
}

fn blocked_response(
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

fn field_error(field: impl Into<String>, message: impl Into<String>) -> CommFieldError {
    CommFieldError {
        field: field.into(),
        message: message.into(),
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

fn strip_empty_optional_values(value: &mut toml::Value) {
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

fn secret_values_present(params: &serde_json::Value) -> bool {
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
