use super::{field_error, CommFieldError};

pub(super) fn validate_schema_fields(protocol: &str, params: &toml::Value) -> Vec<CommFieldError> {
    let mut errors = Vec::new();
    let Some(table) = params.as_table() else {
        return vec![field_error("params", "Parameters must be an object.")];
    };
    match protocol {
        "modbus_tcp" => {
            validate_required_socket_addr(table, "address", &mut errors);
            validate_integer_range(table, "unit_id", 0, 255, &mut errors);
            validate_integer_range(table, "input_start", 0, 65535, &mut errors);
            validate_integer_range(table, "output_start", 0, 65535, &mut errors);
            validate_integer_range(table, "timeout_ms", 1, 60000, &mut errors);
            validate_error_policy(table, &mut errors);
        }
        "mqtt" => {
            validate_required_endpoint(table, "broker", &mut errors);
            validate_integer_range(table, "reconnect_ms", 1, 60000, &mut errors);
            validate_integer_range(table, "keep_alive_s", 1, 65535, &mut errors);
            let has_username = table
                .get("username")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            let has_password = table
                .get("password")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            if has_username ^ has_password {
                errors.push(field_error(
                    "password",
                    "MQTT username and password must be set together.",
                ));
            }
            validate_array_field(table, "tls_alpn", &mut errors);
        }
        "ethercat" => {
            validate_string_field(table, "adapter", true, &mut errors);
            validate_integer_range(table, "timeout_ms", 1, 60000, &mut errors);
            validate_integer_range(table, "cycle_warn_ms", 1, 60000, &mut errors);
            validate_error_policy(table, &mut errors);
            validate_array_field(table, "modules", &mut errors);
            validate_array_field(table, "mock_inputs", &mut errors);
        }
        "gpio" => {
            validate_string_field(table, "sysfs_base", false, &mut errors);
            validate_array_field(table, "inputs", &mut errors);
            validate_array_field(table, "outputs", &mut errors);
        }
        "simulated" => {
            validate_integer_range(table, "input_count", 0, 4096, &mut errors);
            validate_integer_range(table, "output_count", 0, 4096, &mut errors);
            validate_integer_range(table, "scan_period_ms", 1, 60000, &mut errors);
            validate_enum_field(table, "mode", &["static", "counter", "random"], &mut errors);
        }
        "loopback" => {
            validate_integer_range(table, "input_count", 0, 4096, &mut errors);
            validate_integer_range(table, "output_count", 0, 4096, &mut errors);
            validate_integer_range(table, "scan_period_ms", 1, 60000, &mut errors);
            validate_enum_field(table, "mode", &["mirror", "hold_last"], &mut errors);
        }
        _ => errors.push(field_error("protocol", "Unsupported protocol.")),
    }
    errors
}

pub(super) fn validate_runtime_file_fields(
    protocol: &str,
    table: &toml::map::Map<String, toml::Value>,
) -> Vec<CommFieldError> {
    let mut errors = Vec::new();
    match protocol {
        "opcua" => {
            validate_required_endpoint(table, "listen", &mut errors);
            validate_string_field(table, "endpoint_path", true, &mut errors);
            if table
                .get("endpoint_path")
                .and_then(toml::Value::as_str)
                .is_some_and(|path| !path.starts_with('/'))
            {
                errors.push(field_error(
                    "endpoint_path",
                    "Endpoint path must start with '/'.",
                ));
            }
            validate_string_field(table, "namespace_uri", true, &mut errors);
            validate_integer_range(table, "publish_interval_ms", 1, 60000, &mut errors);
            validate_integer_range(table, "max_nodes", 1, 100000, &mut errors);
            validate_array_field(table, "expose", &mut errors);
            validate_enum_field(
                table,
                "security_policy",
                &["basic256sha256", "aes128sha256rsaoaep", "none"],
                &mut errors,
            );
            validate_enum_field(
                table,
                "security_mode",
                &["sign_and_encrypt", "sign", "none"],
                &mut errors,
            );
            let allow_anonymous = table
                .get("allow_anonymous")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            let username = table
                .get("username")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            let password = table
                .get("password")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| !value.trim().is_empty());
            if username ^ password {
                errors.push(field_error(
                    "password",
                    "OPC UA username and password must be set together.",
                ));
            }
            if !allow_anonymous && !username {
                errors.push(field_error(
                    "allow_anonymous",
                    "Enable anonymous access for testing or set username/password.",
                ));
            }
        }
        "openot" => {
            validate_string_field(table, "path", true, &mut errors);
            validate_integer_range(table, "capacity", 1, 1_000_000, &mut errors);
            validate_enum_field(table, "fence_mode", &["fenced", "unfenced"], &mut errors);
            validate_enum_field(table, "source", &["heartbeat", "st-fb"], &mut errors);
            validate_array_field(table, "producer_instances", &mut errors);
            let unfenced = table
                .get("fence_mode")
                .and_then(toml::Value::as_str)
                .is_some_and(|value| value == "unfenced");
            let allow = table
                .get("allow_unfenced_for_proof")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            if unfenced && !allow {
                errors.push(field_error(
                    "allow_unfenced_for_proof",
                    "Unfenced mode requires explicit proof opt-in.",
                ));
            }
        }
        "discovery" => {
            validate_string_field(table, "service_name", true, &mut errors);
            validate_array_field(table, "interfaces", &mut errors);
        }
        "mesh" => {
            validate_enum_field(table, "role", &["peer", "client", "router"], &mut errors);
            validate_required_endpoint(table, "listen", &mut errors);
            validate_array_field(table, "connect", &mut errors);
            validate_array_field(table, "publish", &mut errors);
            validate_table_field(table, "subscribe", &mut errors);
            validate_string_field(table, "zenohd_version", true, &mut errors);
            validate_table_field(table, "plugin_versions", &mut errors);
        }
        "realtime_t0" => {
            validate_enum_field(table, "scheduler", &["fifo", "rr", "other"], &mut errors);
            validate_integer_range(table, "priority", 0, 99, &mut errors);
            validate_array_field(table, "cpu_affinity", &mut errors);
        }
        "runtime_cloud" => {
            validate_enum_field(table, "profile", &["dev", "plant", "wan"], &mut errors);
            validate_array_field(table, "wan_allow_write", &mut errors);
            validate_array_field(table, "link_transports", &mut errors);
        }
        "ads_server" => {
            let enabled = table
                .get("enabled")
                .and_then(toml::Value::as_bool)
                .unwrap_or(false);
            if enabled {
                validate_string_field(table, "listen", true, &mut errors);
            }
            validate_integer_range(table, "ads_port", 1, 65535, &mut errors);
            validate_array_field(table, "expose", &mut errors);
            validate_array_field(table, "writable", &mut errors);
            validate_array_field(table, "clients", &mut errors);
        }
        _ => errors.push(field_error("protocol", "Unsupported protocol.")),
    }
    errors
}

pub(super) fn field_from_error(message: &str) -> &str {
    for field in [
        "address",
        "broker",
        "unit_id",
        "input_start",
        "output_start",
        "timeout_ms",
        "on_error",
        "keep_alive_s",
        "reconnect_ms",
        "modules",
        "inputs",
        "outputs",
        "adapter",
    ] {
        if message.contains(field) {
            return field;
        }
    }
    "_"
}

fn validate_required_socket_addr(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(text) = table.get(field).and_then(toml::Value::as_str) else {
        errors.push(field_error(field, "Enter IP address:port."));
        return;
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        errors.push(field_error(field, "Enter IP address:port."));
        return;
    }
    if trimmed.parse::<std::net::SocketAddr>().is_err() {
        errors.push(field_error(
            field,
            "Use an IP address and port, for example 127.0.0.1:502.",
        ));
    }
}

fn validate_required_endpoint(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(text) = table.get(field).and_then(toml::Value::as_str) else {
        errors.push(field_error(field, "Enter host:port."));
        return;
    };
    let trimmed = text.trim();
    if trimmed.is_empty() {
        errors.push(field_error(field, "Enter host:port."));
        return;
    }
    if !looks_like_host_port(trimmed) {
        errors.push(field_error(
            field,
            "Use host:port, for example 127.0.0.1:502.",
        ));
    }
}

fn looks_like_host_port(text: &str) -> bool {
    let Some((host, port)) = text.rsplit_once(':') else {
        return false;
    };
    let host = host.trim();
    let port = port.trim();
    if host.is_empty() || port.is_empty() {
        return false;
    }
    if host.contains(char::is_whitespace) {
        return false;
    }
    match port.parse::<u16>() {
        Ok(0) | Err(_) => false,
        Ok(_) => true,
    }
}

fn validate_integer_range(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    min: i64,
    max: i64,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get(field) else {
        return;
    };
    let Some(number) = value.as_integer() else {
        errors.push(field_error(field, "Enter a whole number."));
        return;
    };
    if number < min || number > max {
        errors.push(field_error(
            field,
            format!("Enter a value from {min} to {max}."),
        ));
    }
}

fn validate_error_policy(
    table: &toml::map::Map<String, toml::Value>,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get("on_error") else {
        return;
    };
    let Some(text) = value.as_str() else {
        errors.push(field_error("on_error", "Choose fault, warn, or ignore."));
        return;
    };
    if !matches!(text.trim(), "fault" | "warn" | "ignore") {
        errors.push(field_error("on_error", "Choose fault, warn, or ignore."));
    }
}

fn validate_string_field(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    required: bool,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get(field) else {
        if required {
            errors.push(field_error(field, "This field is required."));
        }
        return;
    };
    let Some(text) = value.as_str() else {
        errors.push(field_error(field, "Enter text."));
        return;
    };
    if required && text.trim().is_empty() {
        errors.push(field_error(field, "This field is required."));
    }
}

fn validate_array_field(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get(field) else {
        return;
    };
    if !value.is_array() {
        errors.push(field_error(field, "Enter a JSON array."));
    }
}

fn validate_table_field(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get(field) else {
        return;
    };
    if !value.is_table() {
        errors.push(field_error(field, "Enter a JSON object."));
    }
}

fn validate_enum_field(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    allowed: &[&str],
    errors: &mut Vec<CommFieldError>,
) {
    let Some(value) = table.get(field) else {
        return;
    };
    let Some(text) = value.as_str() else {
        errors.push(field_error(field, "Choose a listed value."));
        return;
    };
    if !allowed.contains(&text.trim()) {
        errors.push(field_error(field, "Choose a listed value."));
    }
}
