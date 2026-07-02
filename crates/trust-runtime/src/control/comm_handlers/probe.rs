use std::net::{TcpStream, ToSocketAddrs};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;

use super::apply::sanitized_apply_audit_details;
use super::contract::COMM_SCHEMA_VERSION;
use super::schema::normalize_protocol;
use super::ControlResponse;

const MAX_TEST_TIMEOUT_MS: u64 = 5_000;

#[derive(Debug, Deserialize)]
struct CommTestRequest {
    protocol: String,
    #[serde(default)]
    params: serde_json::Value,
    credential_channel: Option<String>,
}

#[derive(Debug, Serialize)]
struct CommTestResponse {
    schema_version: u32,
    protocol: String,
    supported: bool,
    ok: bool,
    detail: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<CommProtocolError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    evidence: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    field_errors: Vec<CommFieldError>,
}

#[derive(Debug, Serialize)]
struct CommFieldError {
    field: String,
    message: String,
}

#[derive(Debug, Serialize)]
struct CommProtocolError {
    code: String,
    message: String,
}

pub(super) fn handle_comm_test(id: u64, params: Option<serde_json::Value>) -> ControlResponse {
    let params = match params {
        Some(params) => params,
        None => return ControlResponse::error(id, "missing params".into()),
    };
    let request: CommTestRequest = match serde_json::from_value(params) {
        Ok(value) => value,
        Err(error) => {
            return ControlResponse::error(id, format!("invalid comm.test payload: {error}"))
        }
    };
    let response = test_request(request);
    match serde_json::to_value(response) {
        Ok(value) => ControlResponse::ok(id, value),
        Err(error) => ControlResponse::error(
            id,
            format!("Communication test serialization failed: {error}"),
        ),
    }
}

pub(super) fn probe_value(protocol: &str, params: serde_json::Value) -> serde_json::Value {
    let response = test_request(CommTestRequest {
        protocol: protocol.to_string(),
        params,
        credential_channel: None,
    });
    serde_json::to_value(response).unwrap_or_else(|error| {
        json!({
            "schema_version": COMM_SCHEMA_VERSION,
            "protocol": normalize_protocol(protocol),
            "supported": false,
            "ok": false,
            "detail": format!("Communication test serialization failed: {error}"),
            "error": null,
            "field_errors": [],
        })
    })
}

fn test_request(request: CommTestRequest) -> CommTestResponse {
    let protocol = normalize_protocol(request.protocol.as_str());
    if secret_values_present(&request.params)
        && request.credential_channel.as_deref() != Some("trusted_same_host")
    {
        return blocked(
            protocol,
            "password",
            "Secret fields cannot be sent over an untrusted runtime control channel.",
        );
    }
    match protocol.as_str() {
        "modbus_tcp" => tcp_probe(
            protocol,
            modbus_target(&request.params),
            timeout_ms(&request.params, "timeout_ms"),
            "address",
        ),
        "opcua_client" => opcua_client_probe(protocol, &request.params),
        "mqtt" => tcp_probe(
            protocol,
            mqtt_target(&request.params),
            timeout_ms(&request.params, "timeout_ms"),
            "broker",
        ),
        "simulated" | "loopback" => CommTestResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            supported: true,
            ok: true,
            detail: "This driver does not require a network connection test.".to_string(),
            error: None,
            evidence: Some(json!({ "kind": "local_driver" })),
            field_errors: Vec::new(),
        },
        "ethercat" | "gpio" => CommTestResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            supported: false,
            ok: false,
            detail: "This driver needs runtime hardware health after restart, not a TCP connection test.".to_string(),
            error: None,
            evidence: None,
            field_errors: Vec::new(),
        },
        _ => CommTestResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            supported: false,
            ok: false,
            detail: "This protocol does not have a Communication test yet.".to_string(),
            error: None,
            evidence: None,
            field_errors: Vec::new(),
        },
    }
}

fn opcua_client_probe(protocol: String, params: &serde_json::Value) -> CommTestResponse {
    let target = match opcua_client_target(params) {
        Ok(target) => target,
        Err(error) => {
            return CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: false,
                detail: "OPC UA endpoint test could not start.".to_string(),
                error: None,
                evidence: None,
                field_errors: vec![error],
            }
        }
    };
    match crate::opcua::test_opcua_client_endpoint(
        target.endpoint_url.as_str(),
        target.security,
        target.auth,
        target.trust_server_certificate,
    ) {
        Ok(()) => CommTestResponse {
            schema_version: COMM_SCHEMA_VERSION,
            protocol,
            supported: true,
            ok: true,
            detail: "OPC UA endpoint handshake succeeded.".to_string(),
            error: None,
            evidence: Some(json!({
                "endpoint_url": target.endpoint_url,
                "security_policy": target.security.policy.as_config_value(),
                "security_mode": target.security.mode.as_config_value(),
            })),
            field_errors: Vec::new(),
        },
        Err(error) => {
            let message = format!("OPC UA endpoint handshake failed: {error}");
            CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: false,
                detail: message.clone(),
                error: Some(CommProtocolError {
                    code: crate::opcua::classify_opcua_client_error(&error)
                        .as_str()
                        .to_string(),
                    message,
                }),
                evidence: Some(json!({
                    "endpoint_url": target.endpoint_url,
                    "security_policy": target.security.policy.as_config_value(),
                    "security_mode": target.security.mode.as_config_value(),
                })),
                field_errors: Vec::new(),
            }
        }
    }
}

fn tcp_probe(
    protocol: String,
    target: Result<String, CommFieldError>,
    timeout_ms: u64,
    target_field: &'static str,
) -> CommTestResponse {
    let target = match target {
        Ok(target) => target,
        Err(error) => {
            return CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: false,
                detail: "Connection test could not start.".to_string(),
                error: None,
                evidence: None,
                field_errors: vec![error],
            }
        }
    };
    let timeout = Duration::from_millis(timeout_ms.clamp(1, MAX_TEST_TIMEOUT_MS));
    let resolved = match target
        .to_socket_addrs()
        .ok()
        .and_then(|mut addrs| addrs.next())
    {
        Some(addr) => addr,
        None => {
            return CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: false,
                detail: "Address could not be resolved.".to_string(),
                error: None,
                evidence: Some(json!({ "target": target, "timeout_ms": timeout.as_millis() })),
                field_errors: vec![field_error(target_field, "Enter a resolvable host:port.")],
            }
        }
    };
    match TcpStream::connect_timeout(&resolved, timeout) {
        Ok(_) => {
            let detail = probe_success_detail(protocol.as_str()).to_string();
            CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: true,
                detail,
                error: None,
                evidence: Some(json!({
                    "target": target,
                    "resolved": resolved.to_string(),
                    "timeout_ms": timeout.as_millis()
                })),
                field_errors: Vec::new(),
            }
        }
        Err(error) => {
            let detail = format!("{}: {error}", probe_failure_prefix(protocol.as_str()));
            CommTestResponse {
                schema_version: COMM_SCHEMA_VERSION,
                protocol,
                supported: true,
                ok: false,
                detail,
                error: None,
                evidence: Some(json!({
                    "target": target,
                    "resolved": resolved.to_string(),
                    "timeout_ms": timeout.as_millis()
                })),
                field_errors: Vec::new(),
            }
        }
    }
}

fn probe_success_detail(protocol: &str) -> &'static str {
    match protocol {
        "modbus_tcp" => "Modbus device port is reachable.",
        "mqtt" => "MQTT broker port is reachable.",
        _ => "TCP connection succeeded.",
    }
}

fn probe_failure_prefix(protocol: &str) -> &'static str {
    match protocol {
        "modbus_tcp" => "Modbus device port is not reachable",
        "mqtt" => "MQTT broker port is not reachable",
        _ => "TCP connection failed",
    }
}

fn modbus_target(params: &serde_json::Value) -> Result<String, CommFieldError> {
    let address = params
        .get("address")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if address.is_empty() {
        return Err(field_error("address", "Enter the Modbus device address."));
    }
    Ok(if address.contains(':') {
        address.to_string()
    } else {
        format!("{address}:502")
    })
}

fn mqtt_target(params: &serde_json::Value) -> Result<String, CommFieldError> {
    let broker = params
        .get("broker")
        .or_else(|| params.get("address"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if broker.is_empty() {
        return Err(field_error("broker", "Enter the MQTT broker address."));
    }
    let endpoint = broker
        .strip_prefix("mqtt://")
        .or_else(|| broker.strip_prefix("tcp://"))
        .or_else(|| broker.strip_prefix("ssl://"))
        .unwrap_or(broker);
    Ok(if endpoint.contains(':') {
        endpoint.to_string()
    } else {
        format!("{endpoint}:1883")
    })
}

struct OpcUaClientProbeTarget {
    endpoint_url: String,
    security: crate::opcua::OpcUaSecurityProfile,
    auth: crate::opcua::OpcUaClientAuthConfig,
    trust_server_certificate: bool,
}

fn opcua_client_target(
    params: &serde_json::Value,
) -> Result<OpcUaClientProbeTarget, CommFieldError> {
    let endpoint_url = params
        .get("endpoint_url")
        .or_else(|| params.get("host"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("")
        .trim();
    if endpoint_url.is_empty() {
        return Err(field_error(
            "endpoint_url",
            "Enter the OPC UA endpoint URL or host.",
        ));
    }
    let endpoint_url = if endpoint_url.starts_with("opc.tcp://") {
        endpoint_url.to_string()
    } else if endpoint_url.contains('/')
        || endpoint_url
            .rsplit_once(':')
            .is_some_and(|(_, port)| port.parse::<u16>().is_ok())
    {
        format!("opc.tcp://{endpoint_url}")
    } else {
        format!("opc.tcp://{endpoint_url}:4840")
    };
    let security_policy = params
        .get("security_policy")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("none");
    let security_mode = params
        .get("security_mode")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("none");
    let policy = crate::opcua::OpcUaSecurityPolicy::parse(security_policy).ok_or_else(|| {
        field_error(
            "security_policy",
            format!("Unsupported OPC UA security policy '{security_policy}'."),
        )
    })?;
    let mode = crate::opcua::OpcUaMessageSecurityMode::parse(security_mode).ok_or_else(|| {
        field_error(
            "security_mode",
            format!("Unsupported OPC UA security mode '{security_mode}'."),
        )
    })?;
    let auth_mode = params
        .get("auth")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("anonymous")
        .trim()
        .to_ascii_lowercase();
    let auth = match auth_mode.as_str() {
        "anonymous" => crate::opcua::OpcUaClientAuthConfig::Anonymous,
        "username" | "user_name" | "user" => {
            let username = params
                .get("username")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| field_error("username", "Enter the OPC UA username."))?;
            let password = params
                .get("password")
                .and_then(serde_json::Value::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty())
                .ok_or_else(|| field_error("password", "Enter the OPC UA password."))?;
            crate::opcua::OpcUaClientAuthConfig::UserName {
                username: smol_str::SmolStr::new(username),
                password: smol_str::SmolStr::new(password),
            }
        }
        other => {
            return Err(field_error(
                "auth",
                format!("OPC UA auth must be anonymous or username, got '{other}'."),
            ))
        }
    };
    Ok(OpcUaClientProbeTarget {
        endpoint_url,
        security: crate::opcua::OpcUaSecurityProfile {
            policy,
            mode,
            allow_anonymous: matches!(auth, crate::opcua::OpcUaClientAuthConfig::Anonymous),
        },
        auth,
        trust_server_certificate: params
            .get("trust_server_certificate")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

fn timeout_ms(params: &serde_json::Value, field: &str) -> u64 {
    params
        .get(field)
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(500)
}

fn blocked(protocol: String, field: &str, message: &str) -> CommTestResponse {
    CommTestResponse {
        schema_version: COMM_SCHEMA_VERSION,
        protocol,
        supported: true,
        ok: false,
        detail: "Connection test was blocked.".to_string(),
        error: None,
        evidence: None,
        field_errors: vec![field_error(field, message)],
    }
}

fn field_error(field: impl Into<String>, message: impl Into<String>) -> CommFieldError {
    CommFieldError {
        field: field.into(),
        message: message.into(),
    }
}

fn secret_values_present(params: &serde_json::Value) -> bool {
    sanitized_apply_audit_details(&json!({ "params": params }))
        .get("secret_fields_present")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use std::net::TcpListener;
    use std::thread;

    use super::*;

    #[test]
    fn modbus_probe_reports_tcp_success_and_failure() {
        let address = spawn_one_shot_listener();
        let success = test_request(CommTestRequest {
            protocol: "modbus_tcp".to_string(),
            params: json!({ "address": address, "timeout_ms": 250 }),
            credential_channel: None,
        });
        assert!(success.supported);
        assert!(success.ok, "expected success, got {}", success.detail);
        assert_eq!(success.detail, "Modbus device port is reachable.");

        let refused = unused_loopback_address();
        let failure = test_request(CommTestRequest {
            protocol: "modbus_tcp".to_string(),
            params: json!({ "address": refused, "timeout_ms": 50 }),
            credential_channel: None,
        });
        assert!(failure.supported);
        assert!(!failure.ok);
        assert!(failure
            .detail
            .contains("Modbus device port is not reachable"));
    }

    #[test]
    fn mqtt_probe_reports_tcp_success_and_field_failures() {
        let address = spawn_one_shot_listener();
        let success = test_request(CommTestRequest {
            protocol: "mqtt".to_string(),
            params: json!({ "broker": format!("mqtt://{address}"), "timeout_ms": 250 }),
            credential_channel: None,
        });
        assert!(success.supported);
        assert!(success.ok, "expected success, got {}", success.detail);
        assert_eq!(success.detail, "MQTT broker port is reachable.");

        let missing = test_request(CommTestRequest {
            protocol: "mqtt".to_string(),
            params: json!({}),
            credential_channel: None,
        });
        assert!(missing.supported);
        assert!(!missing.ok);
        assert!(missing
            .field_errors
            .iter()
            .any(|error| error.field == "broker"));

        let unresolved = test_request(CommTestRequest {
            protocol: "mqtt".to_string(),
            params: json!({ "broker": "127.0.0.1:notaport" }),
            credential_channel: None,
        });
        assert!(unresolved.supported);
        assert!(!unresolved.ok);
        assert!(unresolved
            .field_errors
            .iter()
            .any(|error| error.field == "broker"));
    }

    #[cfg(feature = "opcua-wire")]
    #[test]
    fn opcua_client_probe_reports_structured_unreachable_error() {
        let refused = unused_loopback_address();
        let response = test_request(CommTestRequest {
            protocol: "opcua_client".to_string(),
            params: json!({
                "endpoint_url": format!("opc.tcp://{refused}/trust-test"),
                "security_policy": "none",
                "security_mode": "none",
                "auth": "anonymous",
                "trust_server_certificate": true,
            }),
            credential_channel: None,
        });

        assert!(response.supported);
        assert!(!response.ok);
        assert_eq!(
            response.error.as_ref().map(|error| error.code.as_str()),
            Some("endpoint_unreachable")
        );
        assert!(
            response.detail.contains("OPC UA endpoint handshake failed"),
            "detail should remain human-readable"
        );
    }

    fn spawn_one_shot_listener() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address").to_string();
        thread::spawn(move || {
            let _ = listener.accept();
        });
        address
    }

    fn unused_loopback_address() -> String {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind listener");
        let address = listener.local_addr().expect("listener address").to_string();
        drop(listener);
        address
    }
}
