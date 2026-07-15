use std::path::{Path, PathBuf};

use smol_str::SmolStr;
use toml_edit::{value, Array, DocumentMut, InlineTable, Item, Table, Value as EditValue};

use super::validation::validate_runtime_file_fields;
use super::{
    blocked_response, field_error, json_to_toml, secret_values_present,
    strip_empty_optional_values, CommApplyAction, CommApplyRequest, CommApplyResponse,
    CommFieldError,
};
use crate::control::comm_handlers::contract::COMM_SCHEMA_VERSION;

pub(super) fn apply_runtime_protocol(
    project_root: Option<&Path>,
    protocol: String,
    request: CommApplyRequest,
) -> CommApplyResponse {
    if protocol == "ads" {
        return apply_ads_protocol(project_root, protocol, request);
    }
    if protocol == "opcua_client" {
        return apply_opcua_client_protocol(project_root, protocol, request);
    }

    let secret_errors = guard_secret_channel(&request);
    if !secret_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            secret_errors,
            Some("runtime.toml".to_string()),
            None,
        );
    }
    let Some(project_root) = project_root else {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "project_root",
                "No runtime project root is available for runtime.toml setup.",
            )],
            None,
            None,
        );
    };
    let runtime_path = project_root.join("runtime.toml");
    let mut doc = match load_runtime_doc(&runtime_path, &protocol, request.action) {
        Ok(doc) => doc,
        Err(response) => return *response,
    };
    let params = match normalized_params(&request) {
        Ok(params) => params,
        Err(field_errors) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                field_errors,
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    };

    let mut field_errors = Vec::new();
    if !matches!(
        request.action,
        CommApplyAction::Remove | CommApplyAction::Disable
    ) {
        field_errors.extend(validate_runtime_protocol_fields(protocol.as_str(), &params));
    }
    if !field_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            field_errors,
            Some(runtime_path.display().to_string()),
            None,
        );
    }

    match patch_runtime_doc(&mut doc, protocol.as_str(), &params, request.action) {
        Ok(()) => {}
        Err(error) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                vec![error],
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    }

    let patched = doc.to_string();
    if let Err(error) = crate::config::validate_runtime_toml_text(&patched) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error("_", error.to_string())],
            Some(runtime_path.display().to_string()),
            None,
        );
    }

    if request.dry_run || request.action == CommApplyAction::Validate {
        return CommApplyResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            driver: String::new(),
            action: request.action,
            applied: false,
            lifecycle_effect: "validate_only",
            message: "Configuration validated. No files were changed.".to_string(),
            config_path: Some(runtime_path.display().to_string()),
            instance_id: None,
            field_errors: Vec::new(),
            snippet: None,
        };
    }

    if let Err(response) = ensure_parent_dir(&runtime_path, protocol.as_str(), request.action) {
        return *response;
    }
    if let Err(error) = std::fs::write(&runtime_path, patched) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "_",
                format!("failed to write {}: {error}", runtime_path.display()),
            )],
            Some(runtime_path.display().to_string()),
            None,
        );
    }

    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver: String::new(),
        action: request.action,
        applied: true,
        lifecycle_effect: "restart_required",
        message: "Runtime communication configuration saved. Restart the runtime to apply it."
            .to_string(),
        config_path: Some(runtime_path.display().to_string()),
        instance_id: None,
        field_errors: Vec::new(),
        snippet: None,
    }
}

fn apply_opcua_client_protocol(
    project_root: Option<&Path>,
    protocol: String,
    request: CommApplyRequest,
) -> CommApplyResponse {
    let secret_errors = guard_secret_channel(&request);
    if !secret_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            secret_errors,
            Some("opcua_client.toml".to_string()),
            None,
        );
    }
    let Some(project_root) = project_root else {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "project_root",
                "No runtime project root is available for OPC UA client setup.",
            )],
            None,
            None,
        );
    };
    let runtime_path = project_root.join("runtime.toml");
    let mut doc = match load_runtime_doc(&runtime_path, &protocol, request.action) {
        Ok(doc) => doc,
        Err(response) => return *response,
    };
    let params = match normalized_params(&request) {
        Ok(params) => params,
        Err(field_errors) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                field_errors,
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    };

    let enabled = params_bool(&params, "enabled").unwrap_or(true);
    let existing_config_path = runtime_subsection_string(&doc, "opcua_client", "config_path");
    let config_path = params
        .get("config_path")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or(existing_config_path.as_deref())
        .unwrap_or("opcua_client.toml");
    let poll_interval_ms = params
        .get("poll_interval_ms")
        .and_then(toml::Value::as_integer);
    let opcua_client_path = project_relative_path(project_root, config_path);
    let connections = params.get("connections");
    let mut field_errors = Vec::new();
    if enabled
        && !matches!(
            request.action,
            CommApplyAction::Remove | CommApplyAction::Disable | CommApplyAction::Validate
        )
        && connections_array(connections).is_none_or(Vec::is_empty)
        && !opcua_client_path.is_file()
    {
        field_errors.push(field_error(
            "connections",
            "Enable OPC UA client only after adding at least one connection with selected nodes.",
        ));
    }
    if let Some(connections) = connections_array(connections) {
        if enabled && connections.is_empty() {
            field_errors.push(field_error(
                "connections",
                "OPC UA client requires at least one connection with selected nodes.",
            ));
        } else if !connections.is_empty() {
            match render_opcua_client_toml(connections, poll_interval_ms) {
                Ok(text) => {
                    if let Err(error) = crate::opcua::parse_opcua_client_toml(text.as_str()) {
                        field_errors.push(field_error("_", error.to_string()));
                    }
                }
                Err(error) => field_errors.push(error),
            }
        }
    }
    if !field_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            field_errors,
            Some(opcua_client_path.display().to_string()),
            None,
        );
    }

    match patch_opcua_client_runtime_doc(&mut doc, &params, request.action) {
        Ok(()) => {}
        Err(error) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                vec![error],
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    }
    let runtime_text = doc.to_string();
    if let Err(error) = crate::config::validate_runtime_toml_text(runtime_text.as_str()) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error("_", error.to_string())],
            Some(runtime_path.display().to_string()),
            None,
        );
    }
    let opcua_client_text = connections_array(connections)
        .filter(|connections| !connections.is_empty())
        .and_then(|connections| render_opcua_client_toml(connections, poll_interval_ms).ok());

    if request.dry_run || request.action == CommApplyAction::Validate {
        return CommApplyResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            driver: String::new(),
            action: request.action,
            applied: false,
            lifecycle_effect: "validate_only",
            message: "Configuration validated. No files were changed.".to_string(),
            config_path: Some(opcua_client_path.display().to_string()),
            instance_id: None,
            field_errors: Vec::new(),
            snippet: None,
        };
    }
    if let Err(response) = ensure_parent_dir(&runtime_path, protocol.as_str(), request.action) {
        return *response;
    }
    if let Err(error) = std::fs::write(&runtime_path, runtime_text) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "_",
                format!("failed to write {}: {error}", runtime_path.display()),
            )],
            Some(runtime_path.display().to_string()),
            None,
        );
    }
    if let Some(opcua_client_text) = opcua_client_text {
        if let Some(parent) = opcua_client_path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                return blocked_response(
                    protocol,
                    String::new(),
                    request.action,
                    vec![field_error(
                        "_",
                        format!("failed to create {}: {error}", parent.display()),
                    )],
                    Some(opcua_client_path.display().to_string()),
                    None,
                );
            }
        }
        if let Err(error) = std::fs::write(&opcua_client_path, opcua_client_text) {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                vec![field_error(
                    "_",
                    format!("failed to write {}: {error}", opcua_client_path.display()),
                )],
                Some(opcua_client_path.display().to_string()),
                None,
            );
        }
    }
    if request.action == CommApplyAction::Remove {
        match std::fs::remove_file(&opcua_client_path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return blocked_response(
                    protocol,
                    String::new(),
                    request.action,
                    vec![field_error(
                        "_",
                        format!("failed to remove {}: {error}", opcua_client_path.display()),
                    )],
                    Some(opcua_client_path.display().to_string()),
                    None,
                );
            }
        }
    }

    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver: String::new(),
        action: request.action,
        applied: true,
        lifecycle_effect: "restart_required",
        message: "OPC UA client configuration saved. Restart the runtime to apply it.".to_string(),
        config_path: Some(opcua_client_path.display().to_string()),
        instance_id: None,
        field_errors: Vec::new(),
        snippet: None,
    }
}

fn apply_ads_protocol(
    project_root: Option<&Path>,
    protocol: String,
    request: CommApplyRequest,
) -> CommApplyResponse {
    let secret_errors = guard_secret_channel(&request);
    if !secret_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            secret_errors,
            Some("ads.toml".to_string()),
            None,
        );
    }
    let Some(project_root) = project_root else {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "project_root",
                "No runtime project root is available for ADS setup.",
            )],
            None,
            None,
        );
    };
    let runtime_path = project_root.join("runtime.toml");
    let mut doc = match load_runtime_doc(&runtime_path, &protocol, request.action) {
        Ok(doc) => doc,
        Err(response) => return *response,
    };
    let params = match normalized_params(&request) {
        Ok(params) => params,
        Err(field_errors) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                field_errors,
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    };

    let mut field_errors = Vec::new();
    let enabled = params_bool(&params, "enabled").unwrap_or(true);
    let existing_config_path = runtime_subsection_string(&doc, "ads", "config_path");
    let config_path = params
        .get("config_path")
        .and_then(toml::Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .or(existing_config_path.as_deref())
        .unwrap_or("ads.toml");
    let ads_path = project_relative_path(project_root, config_path);
    let requested_connections = connections_array(params.get("connections"));
    let effective_connections = match requested_connections {
        Some(connections) if request.action == CommApplyAction::Add => {
            match merge_ads_connections_for_add(&ads_path, connections) {
                Ok(connections) => Some(connections),
                Err(error) => {
                    field_errors.push(error);
                    None
                }
            }
        }
        Some(connections) => Some(connections.clone()),
        None => None,
    };
    let connections = effective_connections.as_ref();
    if enabled
        && !matches!(
            request.action,
            CommApplyAction::Remove | CommApplyAction::Disable | CommApplyAction::Validate
        )
        && connections.is_none_or(Vec::is_empty)
        && !ads_path.is_file()
    {
        field_errors.push(field_error(
            "connections",
            "Enable ADS only after adding at least one connection.",
        ));
    }
    if let Some(connections) = connections {
        if enabled && connections.is_empty() {
            field_errors.push(field_error(
                "connections",
                "ADS requires at least one connection.",
            ));
        } else if !connections.is_empty() {
            match render_ads_toml(connections) {
                Ok(text) => {
                    if let Err(error) = crate::ads::parse_ads_toml(text.as_str()) {
                        field_errors.push(field_error("_", error.to_string()));
                    }
                }
                Err(error) => field_errors.push(error),
            }
        }
    }
    if !field_errors.is_empty() {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            field_errors,
            Some(ads_path.display().to_string()),
            None,
        );
    }

    match patch_ads_runtime_doc(&mut doc, &params, request.action) {
        Ok(()) => {}
        Err(error) => {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                vec![error],
                Some(runtime_path.display().to_string()),
                None,
            )
        }
    }
    let runtime_text = doc.to_string();
    if let Err(error) = crate::config::validate_runtime_toml_text(runtime_text.as_str()) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error("_", error.to_string())],
            Some(runtime_path.display().to_string()),
            None,
        );
    }
    let ads_text = connections
        .filter(|connections| !connections.is_empty())
        .and_then(|connections| render_ads_toml(connections).ok());

    if request.dry_run || request.action == CommApplyAction::Validate {
        return CommApplyResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            driver: String::new(),
            action: request.action,
            applied: false,
            lifecycle_effect: "validate_only",
            message: "Configuration validated. No files were changed.".to_string(),
            config_path: Some(ads_path.display().to_string()),
            instance_id: None,
            field_errors: Vec::new(),
            snippet: None,
        };
    }
    if let Err(response) = ensure_parent_dir(&runtime_path, protocol.as_str(), request.action) {
        return *response;
    }
    if let Err(error) = std::fs::write(&runtime_path, runtime_text) {
        return blocked_response(
            protocol,
            String::new(),
            request.action,
            vec![field_error(
                "_",
                format!("failed to write {}: {error}", runtime_path.display()),
            )],
            Some(runtime_path.display().to_string()),
            None,
        );
    }
    if let Some(ads_text) = ads_text {
        if let Some(parent) = ads_path.parent() {
            if let Err(error) = std::fs::create_dir_all(parent) {
                return blocked_response(
                    protocol,
                    String::new(),
                    request.action,
                    vec![field_error(
                        "_",
                        format!("failed to create {}: {error}", parent.display()),
                    )],
                    Some(ads_path.display().to_string()),
                    None,
                );
            }
        }
        if let Err(error) = std::fs::write(&ads_path, ads_text) {
            return blocked_response(
                protocol,
                String::new(),
                request.action,
                vec![field_error(
                    "_",
                    format!("failed to write {}: {error}", ads_path.display()),
                )],
                Some(ads_path.display().to_string()),
                None,
            );
        }
    }

    CommApplyResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        driver: String::new(),
        action: request.action,
        applied: true,
        lifecycle_effect: "restart_required",
        message: "ADS configuration saved. Restart the runtime to apply it.".to_string(),
        config_path: Some(ads_path.display().to_string()),
        instance_id: None,
        field_errors: Vec::new(),
        snippet: None,
    }
}

fn load_runtime_doc(
    runtime_path: &Path,
    protocol: &str,
    action: CommApplyAction,
) -> Result<DocumentMut, Box<CommApplyResponse>> {
    let text = match std::fs::read_to_string(runtime_path) {
        Ok(text) => text,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return default_runtime_doc(runtime_path, protocol, action);
        }
        Err(error) => {
            return Err(Box::new(blocked_response(
                protocol.to_string(),
                String::new(),
                action,
                vec![field_error(
                    "runtime.toml",
                    format!("failed to read {}: {error}", runtime_path.display()),
                )],
                Some(runtime_path.display().to_string()),
                None,
            )));
        }
    };
    text.parse::<DocumentMut>().map_err(|error| {
        Box::new(blocked_response(
            protocol.to_string(),
            String::new(),
            action,
            vec![field_error(
                "runtime.toml",
                format!("invalid TOML: {error}"),
            )],
            Some(runtime_path.display().to_string()),
            None,
        ))
    })
}

fn default_runtime_doc(
    runtime_path: &Path,
    protocol: &str,
    action: CommApplyAction,
) -> Result<DocumentMut, Box<CommApplyResponse>> {
    let resource_name = runtime_path
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("runtime");
    crate::bundle_template::render_runtime_toml(&SmolStr::new(resource_name), 10)
        .parse::<DocumentMut>()
        .map_err(|error| {
            Box::new(blocked_response(
                protocol.to_string(),
                String::new(),
                action,
                vec![field_error(
                    "runtime.toml",
                    format!("failed to create default runtime.toml: {error}"),
                )],
                Some(runtime_path.display().to_string()),
                None,
            ))
        })
}

fn ensure_parent_dir(
    path: &Path,
    protocol: &str,
    action: CommApplyAction,
) -> Result<(), Box<CommApplyResponse>> {
    let Some(parent) = path.parent() else {
        return Ok(());
    };
    std::fs::create_dir_all(parent).map_err(|error| {
        Box::new(blocked_response(
            protocol.to_string(),
            String::new(),
            action,
            vec![field_error(
                "_",
                format!("failed to create {}: {error}", parent.display()),
            )],
            Some(path.display().to_string()),
            None,
        ))
    })
}

fn normalized_params(
    request: &CommApplyRequest,
) -> Result<toml::map::Map<String, toml::Value>, Vec<CommFieldError>> {
    let params = match &request.params {
        serde_json::Value::Null => serde_json::Value::Object(Default::default()),
        serde_json::Value::Object(_) => request.params.clone(),
        _ => return Err(vec![field_error("params", "Parameters must be an object.")]),
    };
    let mut params_toml = json_to_toml(&params);
    strip_empty_optional_values(&mut params_toml);
    params_toml
        .as_table()
        .cloned()
        .ok_or_else(|| vec![field_error("params", "Parameters must be a table/object.")])
}

fn guard_secret_channel(request: &CommApplyRequest) -> Vec<CommFieldError> {
    if secret_values_present(&request.params)
        && request.credential_channel.as_deref() != Some("trusted_same_host")
    {
        vec![field_error(
            "password",
            "Secret fields cannot be sent over an untrusted runtime control channel.",
        )]
    } else {
        Vec::new()
    }
}

fn validate_runtime_protocol_fields(
    protocol: &str,
    params: &toml::map::Map<String, toml::Value>,
) -> Vec<CommFieldError> {
    validate_runtime_file_fields(protocol, params)
}

fn patch_runtime_doc(
    doc: &mut DocumentMut,
    protocol: &str,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    let Some(section) = runtime_section(protocol) else {
        return Err(field_error(
            "protocol",
            "Unsupported Communication protocol.",
        ));
    };
    let runtime = ensure_table(doc.as_table_mut(), "runtime")?;
    if action == CommApplyAction::Remove {
        runtime.remove(section);
        return Ok(());
    }
    if protocol == "ads_server" {
        patch_ads_server_runtime_section(runtime, params, action)
    } else if protocol == "runtime_cloud" {
        patch_runtime_cloud(runtime, params, action)
    } else {
        patch_runtime_section(runtime, section, params, action)
    }
}

fn patch_ads_server_runtime_section(
    runtime: &mut Table,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    if action == CommApplyAction::Disable {
        let table = ensure_table(runtime, "ads_server")?;
        table.insert("enabled", value(false));
        return Ok(());
    }

    let previous_clients = runtime
        .get("ads_server")
        .and_then(Item::as_table)
        .and_then(|table| table.get("clients"))
        .cloned();
    let should_preserve_clients = params
        .get("clients")
        .and_then(toml::Value::as_array)
        .is_none_or(Vec::is_empty);

    let mut table = edit_table_from_toml(params);
    if should_preserve_clients {
        if let Some(clients) = previous_clients {
            table.insert("clients", clients);
        }
    }
    runtime.insert("ads_server", Item::Table(table));
    Ok(())
}

fn patch_runtime_section(
    runtime: &mut Table,
    section: &str,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    if action == CommApplyAction::Disable {
        let table = ensure_table(runtime, section)?;
        table.insert("enabled", value(false));
        return Ok(());
    }
    runtime.insert(section, Item::Table(edit_table_from_toml(params)));
    Ok(())
}

fn patch_runtime_cloud(
    runtime: &mut Table,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    if action == CommApplyAction::Disable {
        runtime.remove("cloud");
        return Ok(());
    }
    let mut cloud = Table::new();
    if let Some(profile) = params.get("profile") {
        cloud.insert("profile", item_from_toml(profile));
    }
    if let Some(rules) = params.get("wan_allow_write") {
        let mut wan = Table::new();
        wan.insert("allow_write", item_from_toml(rules));
        cloud.insert("wan", Item::Table(wan));
    }
    if let Some(transports) = params.get("link_transports") {
        let mut links = Table::new();
        links.insert("transports", item_from_toml(transports));
        cloud.insert("links", Item::Table(links));
    }
    runtime.insert("cloud", Item::Table(cloud));
    Ok(())
}

fn patch_ads_runtime_doc(
    doc: &mut DocumentMut,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    let runtime = ensure_table(doc.as_table_mut(), "runtime")?;
    if action == CommApplyAction::Remove {
        runtime.remove("ads");
        return Ok(());
    }
    let table = if action == CommApplyAction::Disable {
        let table = ensure_table(runtime, "ads")?;
        table.insert("enabled", value(false));
        return Ok(());
    } else {
        let mut table = Table::new();
        table.insert(
            "enabled",
            item_from_toml(params.get("enabled").unwrap_or(&toml::Value::Boolean(true))),
        );
        table.insert(
            "config_path",
            item_from_toml(
                params
                    .get("config_path")
                    .unwrap_or(&toml::Value::String("ads.toml".to_string())),
            ),
        );
        table.insert(
            "worker_tick_interval_ms",
            item_from_toml(
                params
                    .get("worker_tick_interval_ms")
                    .unwrap_or(&toml::Value::Integer(20)),
            ),
        );
        table
    };
    runtime.insert("ads", Item::Table(table));
    Ok(())
}

fn patch_opcua_client_runtime_doc(
    doc: &mut DocumentMut,
    params: &toml::map::Map<String, toml::Value>,
    action: CommApplyAction,
) -> Result<(), CommFieldError> {
    let runtime = ensure_table(doc.as_table_mut(), "runtime")?;
    if action == CommApplyAction::Remove {
        runtime.remove("opcua_client");
        return Ok(());
    }
    let table = if action == CommApplyAction::Disable {
        let table = ensure_table(runtime, "opcua_client")?;
        table.insert("enabled", value(false));
        return Ok(());
    } else {
        let mut table = Table::new();
        table.insert(
            "enabled",
            item_from_toml(params.get("enabled").unwrap_or(&toml::Value::Boolean(true))),
        );
        table.insert(
            "config_path",
            item_from_toml(
                params
                    .get("config_path")
                    .unwrap_or(&toml::Value::String("opcua_client.toml".to_string())),
            ),
        );
        table.insert(
            "poll_interval_ms",
            item_from_toml(
                params
                    .get("poll_interval_ms")
                    .unwrap_or(&toml::Value::Integer(250)),
            ),
        );
        table
    };
    runtime.insert("opcua_client", Item::Table(table));
    Ok(())
}

fn runtime_subsection_string(doc: &DocumentMut, section: &str, key: &str) -> Option<String> {
    doc.get("runtime")
        .and_then(Item::as_table)
        .and_then(|runtime| runtime.get(section))
        .and_then(Item::as_table)
        .and_then(|section| section.get(key))
        .and_then(Item::as_value)
        .and_then(EditValue::as_str)
        .map(ToString::to_string)
}

fn runtime_section(protocol: &str) -> Option<&'static str> {
    match protocol {
        "opcua" => Some("opcua"),
        "openot" => Some("openot"),
        "discovery" => Some("discovery"),
        "mesh" => Some("mesh"),
        "realtime_t0" => Some("realtime"),
        "runtime_cloud" => Some("cloud"),
        "ads_server" => Some("ads_server"),
        _ => None,
    }
}

fn ensure_table<'a>(parent: &'a mut Table, key: &str) -> Result<&'a mut Table, CommFieldError> {
    if !parent.contains_key(key) {
        parent.insert(key, Item::Table(Table::new()));
    }
    parent
        .get_mut(key)
        .and_then(Item::as_table_mut)
        .ok_or_else(|| field_error(key, "Expected a TOML table."))
}

fn edit_table_from_toml(map: &toml::map::Map<String, toml::Value>) -> Table {
    let mut table = Table::new();
    for (key, value) in map {
        table.insert(key, item_from_toml(value));
    }
    table
}

fn item_from_toml(value: &toml::Value) -> Item {
    match value {
        toml::Value::Table(table) => Item::Table(edit_table_from_toml(table)),
        other => Item::Value(edit_value_from_toml(other)),
    }
}

fn edit_value_from_toml(value: &toml::Value) -> EditValue {
    match value {
        toml::Value::String(value) => EditValue::from(value.as_str()),
        toml::Value::Integer(value) => EditValue::from(*value),
        toml::Value::Float(value) => EditValue::from(*value),
        toml::Value::Boolean(value) => EditValue::from(*value),
        toml::Value::Datetime(value) => EditValue::from(value.to_string()),
        toml::Value::Array(values) => {
            let mut array = Array::default();
            for value in values {
                array.push(edit_value_from_toml(value));
            }
            EditValue::Array(array)
        }
        toml::Value::Table(table) => {
            let mut inline = InlineTable::default();
            for (key, value) in table {
                inline.insert(key, edit_value_from_toml(value));
            }
            EditValue::InlineTable(inline)
        }
    }
}

fn params_bool(params: &toml::map::Map<String, toml::Value>, key: &str) -> Option<bool> {
    params.get(key).and_then(toml::Value::as_bool)
}

fn connections_array(value: Option<&toml::Value>) -> Option<&Vec<toml::Value>> {
    value.and_then(toml::Value::as_array)
}

fn render_ads_toml(connections: &[toml::Value]) -> Result<String, CommFieldError> {
    let mut root = toml::map::Map::new();
    root.insert(
        "connections".to_string(),
        toml::Value::Array(connections.to_vec()),
    );
    toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|error| field_error("_", format!("failed to render ads.toml: {error}")))
}

fn merge_ads_connections_for_add(
    ads_path: &Path,
    requested: &[toml::Value],
) -> Result<Vec<toml::Value>, CommFieldError> {
    let mut merged = match std::fs::read_to_string(ads_path) {
        Ok(text) => {
            let value = toml::from_str::<toml::Value>(&text).map_err(|error| {
                field_error(
                    "connections",
                    format!("failed to read existing {}: {error}", ads_path.display()),
                )
            })?;
            value
                .get("connections")
                .and_then(toml::Value::as_array)
                .cloned()
                .ok_or_else(|| {
                    field_error(
                        "connections",
                        format!("{} has no connections array", ads_path.display()),
                    )
                })?
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => {
            return Err(field_error(
                "connections",
                format!("failed to read {}: {error}", ads_path.display()),
            ));
        }
    };
    for connection in requested {
        let identity = ads_connection_identity(connection);
        let already_present = identity.as_ref().is_some_and(|identity| {
            merged
                .iter()
                .any(|existing| ads_connection_identity(existing).as_ref() == Some(identity))
        });
        if !already_present {
            merged.push(connection.clone());
        }
    }
    Ok(merged)
}

fn ads_connection_identity(value: &toml::Value) -> Option<(String, String, i64)> {
    let table = value.as_table()?;
    let target_net_id = table.get("target_net_id")?.as_str()?.trim();
    let host = table.get("host")?.as_str()?.trim();
    if target_net_id.is_empty() || host.is_empty() {
        return None;
    }
    let port = table
        .get("ams_port")
        .and_then(toml::Value::as_integer)
        .unwrap_or(851);
    Some((target_net_id.to_string(), host.to_string(), port))
}

fn render_opcua_client_toml(
    connections: &[toml::Value],
    default_poll_interval_ms: Option<i64>,
) -> Result<String, CommFieldError> {
    let connections = connections
        .iter()
        .cloned()
        .map(|mut connection| {
            if let (Some(default_poll_interval_ms), Some(table)) =
                (default_poll_interval_ms, connection.as_table_mut())
            {
                table
                    .entry("poll_interval_ms")
                    .or_insert(toml::Value::Integer(default_poll_interval_ms));
            }
            connection
        })
        .collect::<Vec<_>>();
    let mut root = toml::map::Map::new();
    root.insert("connections".to_string(), toml::Value::Array(connections));
    toml::to_string_pretty(&toml::Value::Table(root))
        .map_err(|error| field_error("_", format!("failed to render opcua_client.toml: {error}")))
}

fn project_relative_path(project_root: &Path, path: &str) -> PathBuf {
    let path = PathBuf::from(path);
    if path.is_relative() {
        project_root.join(path)
    } else {
        path
    }
}
