use trust_runtime_services::io_drivers::modbus::ModbusTcpParams;
use trust_runtime_services::io_drivers::mqtt::MqttParams;
use trust_runtime_services::io_drivers::ParamsError;

use super::{field_error, CommFieldError};

pub(super) fn validate_schema_fields(protocol: &str, params: &toml::Value) -> Vec<CommFieldError> {
    let mut errors = Vec::new();
    let Some(table) = params.as_table() else {
        return vec![field_error("params", "Parameters must be an object.")];
    };
    match protocol {
        // The driver's own rules (specification 11 §6.5; specification 32 §2), whose bounds
        // `comm.schema` advertises for their number fields.
        "modbus_tcp" => errors.extend(driver_rules(params, ModbusTcpParams::refusals)),
        "mqtt" => errors.extend(driver_rules(params, MqttParams::refusals)),
        "ethercat" => {
            validate_string_field(table, "adapter", true, &mut errors);
            validate_integer_range(table, "timeout_ms", 1, 60000, &mut errors);
            validate_integer_range(table, "cycle_warn_ms", 1, 60000, &mut errors);
            validate_error_policy(table, &mut errors);
            validate_array_field(table, "modules", &mut errors);
            validate_array_field(table, "mock_inputs", &mut errors);
            validate_array_field(table, "selected_channels", &mut errors);
        }
        "gpio" => {
            validate_enum_field(
                table,
                "backend",
                &["libgpiod", "chardev", "sysfs"],
                &mut errors,
            );
            validate_string_field(table, "chip", false, &mut errors);
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
            validate_integer_range(table, "max_symbols", 1, 1_000_000, &mut errors);
            validate_integer_range(table, "max_clients", 1, 10_000, &mut errors);
            validate_integer_range(
                table,
                "max_subscriptions_per_client",
                1,
                1_000_000,
                &mut errors,
            );
            validate_integer_range(table, "max_total_subscriptions", 1, 10_000_000, &mut errors);
            validate_integer_range(
                table,
                "max_frame_bytes",
                1024,
                64 * 1024 * 1024,
                &mut errors,
            );
            validate_integer_range(table, "max_sumup_items", 1, 65535, &mut errors);
            validate_integer_range(table, "max_write_bytes", 1, 64 * 1024 * 1024, &mut errors);
            validate_array_field(table, "expose", &mut errors);
            validate_array_field(table, "writable", &mut errors);
            validate_array_field(table, "clients", &mut errors);
        }
        _ => errors.push(field_error("protocol", "Unsupported protocol.")),
    }
    errors
}

/// The refusals of a driver's own rules, shared by every runtime, on the parameters as the apply
/// path writes them, without empty optional values. The bounds that `comm.schema` advertises for
/// the driver's number fields are the driver's own (specification 32 §2), so these refusals
/// enforce them.
fn driver_rules(
    params: &toml::Value,
    refusals: fn(&str) -> Vec<ParamsError>,
) -> Vec<CommFieldError> {
    let mut applied = params.clone();
    super::strip_empty_optional_values(&mut applied);
    shared_refusals(&applied, refusals)
}

/// The refusals of `params` by the shared rules `refusals`, every one at once, each a field error
/// on the first key of the path it names, in the words that the runtime refuses the driver with
/// (`<key>: <reason>`).
fn shared_refusals(
    params: &toml::Value,
    refusals: fn(&str) -> Vec<ParamsError>,
) -> Vec<CommFieldError> {
    match crate::io::params_text(params) {
        Ok(text) => refusals(&text)
            .into_iter()
            .map(|refusal| {
                let field = refusal.key.split(['[', '.']).next().unwrap_or_default();
                field_error(field, refusal.to_string())
            })
            .collect(),
        Err(reason) => vec![field_error("params", reason)],
    }
}

pub(super) fn field_from_error(message: &str) -> &str {
    for field in [
        // The computer's refusals of an MQTT driver's TLS files name the key first, before a path
        // that might hold another key's name.
        "tls_ca_path",
        "tls_client_cert_path",
        "tls_client_key_path",
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

fn validate_required_endpoint(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    errors: &mut Vec<CommFieldError>,
) {
    validate_required_endpoint_with_example(table, field, "127.0.0.1:502", errors);
}

fn validate_required_endpoint_with_example(
    table: &toml::map::Map<String, toml::Value>,
    field: &'static str,
    example: &str,
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
            format!("Use host:port, for example {example}."),
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

#[cfg(test)]
mod tests {
    use super::validate_schema_fields;

    fn mqtt_errors(params: toml::Value) -> Vec<(String, String)> {
        validate_schema_fields("mqtt", &params)
            .into_iter()
            .map(|error| (error.field, error.message))
            .collect()
    }

    fn modbus_errors(params: toml::Value) -> Vec<(String, String)> {
        validate_schema_fields("modbus_tcp", &params)
            .into_iter()
            .map(|error| (error.field, error.message))
            .collect()
    }

    /// Specification 32 §2 and 11 §6.5: a connection form takes what a hand-written `io.toml` runs with, the driver's own bounds. An
    /// MQTT reconnect delay of 2 minutes and a keep alive of 0, which the driver reads as 1 s, and a Modbus timeout of 2 minutes are
    /// taken; a keep alive of 65,536 s and a timeout of 0 ms are refused. Catches a form whose bounds are narrower than the driver's,
    /// so that a device's settings that run are refused on the form's next save.
    #[test]
    fn the_form_takes_the_settings_the_driver_runs_with() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                reconnect_ms = 120000
                keep_alive_s = 0
            }
            .into(),
        );
        assert!(errors.is_empty(), "{errors:?}");
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                keep_alive_s = 65536
            }
            .into(),
        );
        assert_eq!(
            errors
                .iter()
                .map(|(field, _)| field.as_str())
                .collect::<Vec<_>>(),
            ["keep_alive_s"],
            "{errors:?}"
        );
        let errors = modbus_errors(
            toml::toml! {
                address = "127.0.0.1:502"
                timeout_ms = 120000
            }
            .into(),
        );
        assert!(errors.is_empty(), "{errors:?}");
        let errors = modbus_errors(
            toml::toml! {
                address = "127.0.0.1:502"
                timeout_ms = 0
            }
            .into(),
        );
        assert_eq!(
            errors
                .iter()
                .map(|(field, _)| field.as_str())
                .collect::<Vec<_>>(),
            ["timeout_ms"],
            "{errors:?}"
        );
    }

    /// Every key that the Modbus TCP driver refuses is marked at once, in the order of its rules
    /// (specification 11 §6.5): an identity that is no text, port 0, a unit above 255, a timeout of
    /// 0 ms, an unknown policy and an unknown output function. Catches a form that marks only the
    /// first refused key, so that each save finds one more, and a port 0 that the form took and the
    /// driver refused.
    #[test]
    fn every_key_the_modbus_driver_refuses_is_marked_at_once() {
        let errors = modbus_errors(
            toml::toml! {
                address = "10.0.0.5:0"
                product_code = 7
                unit_id = 256
                on_error = "explode"
                output_function = "fc99"
                timeout_ms = 0
            }
            .into(),
        );
        let fields: Vec<&str> = errors.iter().map(|(field, _)| field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "product_code",
                "address",
                "unit_id",
                "timeout_ms",
                "on_error",
                "output_function"
            ],
            "{errors:?}"
        );
    }

    /// The form reads a Modbus TCP policy as the driver reads it: trimmed, in any case, with
    /// `warning` for `warn` (specification 11 §6.5). Catches a form that refuses an `io.toml` the
    /// runtime runs with.
    #[test]
    fn the_modbus_policy_is_read_as_the_driver_reads_it() {
        for policy in [" Warning ", "IGNORE", "Fault"] {
            let mut params = toml::Table::new();
            params.insert("address".into(), "10.0.0.5:502".into());
            params.insert("on_error".into(), policy.into());
            let errors = modbus_errors(params.into());
            assert!(errors.is_empty(), "{policy}: {errors:?}");
        }
    }

    /// The field error of `field` in the driver's own words.
    fn on(field: &str, message: &str) -> (String, String) {
        (field.to_owned(), message.to_owned())
    }

    /// TLS without an authority marks `tls_ca_path` with the driver's refusal (specification 32
    /// §3). Catches a form that lets TLS on without the broker's authority through to the save.
    #[test]
    fn mqtt_tls_without_ca_path_is_field_specific() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                topic_in = "trust/io/in"
                topic_out = "trust/io/out"
                tls = true
                reconnect_ms = 500
                keep_alive_s = 5
                tls_alpn = []
            }
            .into(),
        );
        assert_eq!(
            errors,
            [on(
                "tls_ca_path",
                "tls_ca_path: is required with TLS, the path of the PEM file of the broker's \
                 certificate authority"
            )]
        );
    }

    /// A broker without a port marks `broker` with the driver's refusal and nothing else: its
    /// scheme still lets the TLS rules judge the TLS keys, which are in order (specification 32
    /// §2). Catches a broker refusal on another field.
    #[test]
    fn mqtt_broker_without_a_port_is_refused_on_its_field() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "not-a-host"
                topic_in = "trust/io/in"
                topic_out = "trust/io/out"
                tls = false
                reconnect_ms = 500
                keep_alive_s = 5
                tls_alpn = []
            }
            .into(),
        );
        assert_eq!(
            errors,
            [on(
                "broker",
                "broker: must be host:port, not \"not-a-host\""
            )]
        );
    }

    /// A TLS file named with TLS off marks that file's key (specification 32 §3). Catches a TLS
    /// setting that a form takes and the driver then ignores.
    #[test]
    fn mqtt_tls_fields_require_tls_enabled() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                topic_in = "trust/io/in"
                topic_out = "trust/io/out"
                tls = false
                reconnect_ms = 500
                keep_alive_s = 5
                tls_ca_path = "/tmp/ca.pem"
                tls_alpn = []
            }
            .into(),
        );
        assert_eq!(
            errors,
            [on(
                "tls_ca_path",
                "tls_ca_path: needs TLS: set tls = true or use an mqtts:// or ssl:// broker"
            )]
        );
    }

    /// Half a mutual-TLS pair marks the missing half (specification 32 §3). Catches a client that
    /// would connect without its identity.
    #[test]
    fn mqtt_mtls_cert_and_key_are_validated_together() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                topic_in = "trust/io/in"
                topic_out = "trust/io/out"
                tls = true
                reconnect_ms = 500
                keep_alive_s = 5
                tls_ca_path = "/tmp/ca.pem"
                tls_client_cert_path = "/tmp/client.pem"
                tls_alpn = []
            }
            .into(),
        );
        assert_eq!(
            errors,
            [on(
                "tls_client_key_path",
                "tls_client_key_path: is required with tls_client_cert_path: mutual TLS takes both"
            )]
        );
    }

    /// Every key that the driver refuses is marked at once, in the order of its rules
    /// (specification 32 §2): the missing authority, the missing half of the pair, an empty ALPN
    /// protocol on `tls_alpn` (its refusal names `tls_alpn[1]`), the missing password, a keep alive
    /// above 65535 and an unknown policy. Catches a form that marks only the first refused key, so
    /// that each save finds one more, and a refusal of an item marked on no field of the form.
    #[test]
    fn every_key_the_driver_refuses_is_marked_at_once() {
        let errors = mqtt_errors(
            toml::toml! {
                broker = "127.0.0.1:1883"
                tls = true
                tls_client_cert_path = "/tmp/client.pem"
                tls_alpn = ["mqtt", ""]
                username = "line"
                keep_alive_s = 70000
                on_error = "explode"
            }
            .into(),
        );
        let fields: Vec<&str> = errors.iter().map(|(field, _)| field.as_str()).collect();
        assert_eq!(
            fields,
            [
                "tls_ca_path",
                "tls_client_key_path",
                "tls_alpn",
                "password",
                "keep_alive_s",
                "on_error"
            ],
            "{errors:?}"
        );
    }

    #[test]
    fn ethercat_selected_channels_is_an_allowed_array_param() {
        let errors = validate_schema_fields(
            "ethercat",
            &toml::toml! {
                adapter = "mock"
                timeout_ms = 250
                cycle_warn_ms = 5
                on_error = "fault"
                modules = [
                    { model = "EK1100", slot = 0, channels = 1 },
                    { model = "EL1008", slot = 1, channels = 8 },
                ]
                mock_inputs = []
                selected_channels = ["ethercat.slot1.channel0"]
            }
            .into(),
        );

        assert!(
            errors.is_empty(),
            "selected EtherCAT channels must validate as driver params: {errors:?}"
        );
    }
}
