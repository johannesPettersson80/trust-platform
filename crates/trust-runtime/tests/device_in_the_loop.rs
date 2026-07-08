use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use native_tls::{Certificate, Identity, TlsConnector};
use rumqttc::{Client, Event, MqttOptions, Outgoing, Packet, QoS, TlsConfiguration, Transport};
use serde_json::{json, Value};
use trust_runtime::io::discover_ethercat_modules;

const SCHEMA_VERSION: u32 = 1;

#[test]
#[ignore = "requires configured lab EtherCAT hardware; see docs/internal/testing/runtime-device-in-the-loop.md"]
fn ethercat_lab_hardware_discovery_records_topology() {
    let Some(adapter) = env_text("TRUST_DIT_ETHERCAT_ADAPTER") else {
        skip_artifact(
            "ethercat",
            "ethercat-discovery.json",
            &["TRUST_DIT_ETHERCAT_ADAPTER"],
            "no lab EtherCAT adapter configured",
        );
        return;
    };

    let params = match ethercat_params(&adapter) {
        Ok(params) => params,
        Err(error) => {
            fail_artifact(
                "ethercat",
                "ethercat-discovery.json",
                "invalid EtherCAT device-in-the-loop parameters",
                json!({ "error": error }),
            );
        }
    };

    match discover_ethercat_modules(&params) {
        Ok(discovery) => {
            write_artifact(
                "ethercat-discovery.json",
                json!({
                    "schema_version": SCHEMA_VERSION,
                    "protocol": "ethercat",
                    "status": "pass",
                    "adapter": adapter,
                    "timestamp_ms": now_ms(),
                    "input_bytes": discovery.input_bytes,
                    "output_bytes": discovery.output_bytes,
                    "modules": discovery.modules.into_iter().map(|module| {
                        json!({
                            "model": module.model,
                            "slot": module.slot,
                            "channels": module.channels,
                        })
                    }).collect::<Vec<_>>(),
                }),
            );
        }
        Err(error) => {
            fail_artifact(
                "ethercat",
                "ethercat-discovery.json",
                "EtherCAT lab discovery failed",
                json!({
                    "adapter": adapter,
                    "error": error.to_string(),
                }),
            );
        }
    }
}

#[test]
#[ignore = "requires explicit lab EtherCAT storage stress opt-in; see docs/internal/testing/runtime-device-in-the-loop.md"]
fn ethercat_lab_pdu_storage_stress_records_artifact() {
    let Some(adapter) = env_text("TRUST_DIT_ETHERCAT_ADAPTER") else {
        skip_artifact(
            "ethercat",
            "ethercat-storage-stress.json",
            &["TRUST_DIT_ETHERCAT_ADAPTER"],
            "no lab EtherCAT adapter configured",
        );
        return;
    };
    if !env_bool("TRUST_DIT_ETHERCAT_STORAGE_STRESS") {
        skip_artifact(
            "ethercat",
            "ethercat-storage-stress.json",
            &["TRUST_DIT_ETHERCAT_STORAGE_STRESS"],
            "EtherCAT storage stress requires explicit opt-in",
        );
        return;
    }

    let params = match ethercat_params(&adapter) {
        Ok(params) => params,
        Err(error) => {
            fail_artifact(
                "ethercat",
                "ethercat-storage-stress.json",
                "invalid EtherCAT device-in-the-loop parameters",
                json!({ "error": error }),
            );
        }
    };
    let cycles = env_text("TRUST_DIT_ETHERCAT_STORAGE_STRESS_CYCLES")
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(3)
        .clamp(1, 20);
    let initial_rss = current_rss_kib();
    let mut max_rss = initial_rss;
    let mut cycle_reports = Vec::new();

    for cycle in 0..cycles {
        let before_rss = current_rss_kib();
        match discover_ethercat_modules(&params) {
            Ok(discovery) => {
                let after_rss = current_rss_kib();
                max_rss = max_optional(max_rss, after_rss);
                cycle_reports.push(json!({
                    "cycle": cycle,
                    "status": "pass",
                    "before_rss_kib": before_rss,
                    "after_rss_kib": after_rss,
                    "input_bytes": discovery.input_bytes,
                    "output_bytes": discovery.output_bytes,
                    "modules": discovery.modules.into_iter().map(|module| {
                        json!({
                            "model": module.model,
                            "slot": module.slot,
                            "channels": module.channels,
                        })
                    }).collect::<Vec<_>>(),
                }));
            }
            Err(error) => {
                fail_artifact(
                    "ethercat",
                    "ethercat-storage-stress.json",
                    "EtherCAT storage stress failed",
                    json!({
                        "adapter": adapter,
                        "cycle": cycle,
                        "cycles_requested": cycles,
                        "initial_rss_kib": initial_rss,
                        "before_rss_kib": before_rss,
                        "max_rss_kib": max_rss,
                        "cycles": cycle_reports,
                        "error": error.to_string(),
                    }),
                );
            }
        }
    }

    write_artifact(
        "ethercat-storage-stress.json",
        json!({
            "schema_version": SCHEMA_VERSION,
            "protocol": "ethercat",
            "status": "pass",
            "adapter": adapter,
            "timestamp_ms": now_ms(),
            "cycles_requested": cycles,
            "initial_rss_kib": initial_rss,
            "final_rss_kib": current_rss_kib(),
            "max_rss_kib": max_rss,
            "cycles": cycle_reports,
        }),
    );
}

#[test]
#[ignore = "requires configured lab TwinCAT/ADS target; see docs/internal/testing/runtime-device-in-the-loop.md"]
fn ads_lab_twincat_doctor_records_status_json() {
    let Some(target) = env_text("TRUST_DIT_ADS_TARGET") else {
        skip_artifact(
            "ads",
            "ads-doctor.json",
            &["TRUST_DIT_ADS_TARGET"],
            "no lab ADS target configured",
        );
        return;
    };

    let mut args = vec![
        "ads".to_string(),
        "doctor".to_string(),
        "--target".to_string(),
        target.clone(),
        "--json".to_string(),
    ];
    if let Some(target_net_id) = env_text("TRUST_DIT_ADS_TARGET_NET_ID") {
        args.push("--target-net-id".to_string());
        args.push(target_net_id);
    }
    if let Some(ams_port) = env_text("TRUST_DIT_ADS_AMS_PORT") {
        args.push("--ams-port".to_string());
        args.push(ams_port);
    }
    match (
        env_text("TRUST_DIT_ADS_WRITE_SYMBOL"),
        env_text("TRUST_DIT_ADS_WRITE_TYPE"),
        env_text("TRUST_DIT_ADS_WRITE_VALUE"),
    ) {
        (Some(symbol), Some(type_name), Some(value)) => {
            args.push("--write-symbol".to_string());
            args.push(symbol);
            args.push("--write-type".to_string());
            args.push(type_name);
            args.push("--write-value".to_string());
            args.push(value);
        }
        (None, None, None) => {}
        _ => {
            fail_artifact(
                "ads",
                "ads-doctor.json",
                "ADS guarded write probe is partially configured",
                json!({
                    "required_group": [
                        "TRUST_DIT_ADS_WRITE_SYMBOL",
                        "TRUST_DIT_ADS_WRITE_TYPE",
                        "TRUST_DIT_ADS_WRITE_VALUE"
                    ],
                }),
            );
        }
    }

    let report = run_runtime_cli(&args);
    let stdout_json = parse_stdout_json(&report);
    let overall = stdout_json
        .as_ref()
        .and_then(|value| value.get("overall"))
        .and_then(Value::as_str)
        .unwrap_or("unknown");
    let require_production_ready = env_bool("TRUST_DIT_ADS_REQUIRE_PRODUCTION_READY");
    let accepted = overall == "pass" || (!require_production_ready && overall == "partial");
    write_artifact(
        "ads-doctor.json",
        json!({
            "schema_version": SCHEMA_VERSION,
            "protocol": "ads",
            "status": if report.status_success && accepted { "pass" } else { "fail" },
            "target": target,
            "timestamp_ms": now_ms(),
            "require_production_ready": require_production_ready,
            "accepted_overall": ["pass", "partial"],
            "command": report.command,
            "exit_code": report.exit_code,
            "stdout": report.stdout,
            "stderr": report.stderr,
            "doctor": stdout_json,
        }),
    );
    assert!(
        report.status_success,
        "ADS doctor command failed; see {}",
        artifact_path("ads-doctor.json").display()
    );
    assert!(
        accepted,
        "ADS doctor overall '{overall}' is not accepted by this lab gate; see {}",
        artifact_path("ads-doctor.json").display()
    );
}

#[test]
#[ignore = "requires configured lab Modbus target; see docs/internal/testing/runtime-device-in-the-loop.md"]
fn modbus_lab_target_confirms_protocol_probe() {
    let Some(host) = env_text("TRUST_DIT_MODBUS_HOST") else {
        skip_artifact(
            "modbus_tcp",
            "modbus-discovery.json",
            &["TRUST_DIT_MODBUS_HOST"],
            "no lab Modbus target configured",
        );
        return;
    };

    let mut args = vec![
        "comm".to_string(),
        "discover".to_string(),
        "--protocol".to_string(),
        "modbus-tcp".to_string(),
        "--origin".to_string(),
        "this-host".to_string(),
        "--host".to_string(),
        host.clone(),
        "--timeout-ms".to_string(),
        env_text("TRUST_DIT_MODBUS_TIMEOUT_MS").unwrap_or_else(|| "1500".to_string()),
        "--json".to_string(),
    ];
    if let Some(unit_id) = env_text("TRUST_DIT_MODBUS_UNIT_ID") {
        args.push("--unit-id".to_string());
        args.push(unit_id);
    }
    match (
        env_text("TRUST_DIT_MODBUS_PROBE_READ_ADDRESS"),
        env_text("TRUST_DIT_MODBUS_PROBE_READ_QUANTITY"),
    ) {
        (Some(address), quantity) => {
            args.push("--probe-read-address".to_string());
            args.push(address);
            if let Some(quantity) = quantity {
                args.push("--probe-read-quantity".to_string());
                args.push(quantity);
            }
        }
        (None, Some(_)) => {
            fail_artifact(
                "modbus_tcp",
                "modbus-discovery.json",
                "Modbus safe-read quantity was configured without an address",
                json!({
                    "required": "TRUST_DIT_MODBUS_PROBE_READ_ADDRESS",
                }),
            );
        }
        (None, None) => {}
    }

    let report = run_runtime_cli(&args);
    let stdout_json = parse_stdout_json(&report);
    let confirmed = stdout_json
        .as_ref()
        .and_then(|value| value.get("candidates"))
        .and_then(Value::as_array)
        .is_some_and(|candidates| {
            candidates.iter().any(|candidate| {
                candidate.get("confidence").and_then(Value::as_str) == Some("confirmed")
            })
        });
    write_artifact(
        "modbus-discovery.json",
        json!({
            "schema_version": SCHEMA_VERSION,
            "protocol": "modbus_tcp",
            "status": if report.status_success && confirmed { "pass" } else { "fail" },
            "target": host,
            "timestamp_ms": now_ms(),
            "command": report.command,
            "exit_code": report.exit_code,
            "stdout": report.stdout,
            "stderr": report.stderr,
            "discovery": stdout_json,
        }),
    );
    assert!(
        report.status_success,
        "Modbus discovery command failed; see {}",
        artifact_path("modbus-discovery.json").display()
    );
    assert!(
        confirmed,
        "Modbus lab gate requires confirmed protocol evidence, not only TCP reachability; see {}",
        artifact_path("modbus-discovery.json").display()
    );
}

#[test]
#[ignore = "requires configured lab MQTT broker; see docs/internal/testing/runtime-device-in-the-loop.md"]
fn mqtt_lab_broker_records_auth_tls_reconnect_and_disconnect() {
    let Some(broker) = env_text("TRUST_DIT_MQTT_BROKER") else {
        skip_artifact(
            "mqtt",
            "mqtt-interop.json",
            &["TRUST_DIT_MQTT_BROKER"],
            "no lab MQTT broker configured",
        );
        return;
    };

    let endpoint = match parse_mqtt_broker(&broker) {
        Ok(endpoint) => endpoint,
        Err(error) => {
            fail_artifact(
                "mqtt",
                "mqtt-interop.json",
                "invalid MQTT broker target",
                json!({ "broker": broker, "error": error }),
            );
        }
    };
    let credentials = match (
        env_text("TRUST_DIT_MQTT_USERNAME"),
        env_text("TRUST_DIT_MQTT_PASSWORD"),
    ) {
        (Some(username), Some(password)) => Some((username, password)),
        (None, None) => None,
        _ => {
            fail_artifact(
                "mqtt",
                "mqtt-interop.json",
                "MQTT username/password must be configured together",
                json!({
                    "required_group": ["TRUST_DIT_MQTT_USERNAME", "TRUST_DIT_MQTT_PASSWORD"],
                }),
            );
        }
    };
    let topic = env_text("TRUST_DIT_MQTT_TOPIC")
        .unwrap_or_else(|| format!("trust/device-in-loop/{}", std::process::id()));
    let cycles = env_text("TRUST_DIT_MQTT_RECONNECT_CYCLES")
        .and_then(|text| text.parse::<usize>().ok())
        .unwrap_or(2)
        .max(2);
    let mut cycle_reports = Vec::new();
    for cycle in 0..cycles {
        match mqtt_publish_cycle(cycle, &endpoint, credentials.as_ref(), &topic) {
            Ok(report) => cycle_reports.push(report),
            Err(error) => {
                write_artifact(
                    "mqtt-interop.json",
                    json!({
                        "schema_version": SCHEMA_VERSION,
                        "protocol": "mqtt",
                        "status": "fail",
                        "timestamp_ms": now_ms(),
                        "broker": endpoint.redacted(),
                        "topic": topic,
                        "credentials_supplied": credentials.is_some(),
                        "tls_requested": endpoint.tls,
                        "clean_session_requested": true,
                        "reconnect_cycles_requested": cycles,
                        "cycles": cycle_reports,
                        "error": error,
                    }),
                );
                panic!(
                    "MQTT broker interop failed; see {}",
                    artifact_path("mqtt-interop.json").display()
                );
            }
        }
    }

    write_artifact(
        "mqtt-interop.json",
        json!({
            "schema_version": SCHEMA_VERSION,
            "protocol": "mqtt",
            "status": "pass",
            "timestamp_ms": now_ms(),
            "broker": endpoint.redacted(),
            "topic": topic,
            "credentials_supplied": credentials.is_some(),
            "tls_requested": endpoint.tls,
            "clean_session_requested": true,
            "explicit_disconnect": true,
            "reconnect_cycles_requested": cycles,
            "cycles": cycle_reports,
        }),
    );
}

fn ethercat_params(adapter: &str) -> Result<toml::Value, String> {
    if let Some(path) = env_text("TRUST_DIT_ETHERCAT_PARAMS_FILE") {
        let text = fs::read_to_string(&path)
            .map_err(|error| format!("read TRUST_DIT_ETHERCAT_PARAMS_FILE '{path}': {error}"))?;
        return toml::from_str(&text)
            .map_err(|error| format!("parse TRUST_DIT_ETHERCAT_PARAMS_FILE '{path}': {error}"));
    }
    if let Some(text) = env_text("TRUST_DIT_ETHERCAT_PARAMS_TOML") {
        return toml::from_str(&text)
            .map_err(|error| format!("parse TRUST_DIT_ETHERCAT_PARAMS_TOML: {error}"));
    }
    let mut table = toml::map::Map::new();
    table.insert(
        "adapter".to_string(),
        toml::Value::String(adapter.to_string()),
    );
    Ok(toml::Value::Table(table))
}

#[derive(Debug)]
struct CliReport {
    command: Vec<String>,
    exit_code: Option<i32>,
    status_success: bool,
    stdout: String,
    stderr: String,
}

fn run_runtime_cli(args: &[String]) -> CliReport {
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(args)
        .output()
        .unwrap_or_else(|error| panic!("run trust-runtime {}: {error}", args.join(" ")));
    CliReport {
        command: std::iter::once("trust-runtime".to_string())
            .chain(args.iter().cloned())
            .collect(),
        exit_code: output.status.code(),
        status_success: output.status.success(),
        stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
        stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
    }
}

fn parse_stdout_json(report: &CliReport) -> Option<Value> {
    serde_json::from_str(report.stdout.trim()).ok()
}

#[derive(Debug)]
struct MqttEndpoint {
    host: String,
    port: u16,
    tls: bool,
}

impl MqttEndpoint {
    fn redacted(&self) -> Value {
        json!({
            "host": self.host,
            "port": self.port,
            "tls": self.tls,
        })
    }
}

fn parse_mqtt_broker(text: &str) -> Result<MqttEndpoint, String> {
    let trimmed = text.trim();
    let (remainder, scheme_tls) = if let Some(rest) = trimmed.strip_prefix("mqtts://") {
        (rest, true)
    } else if let Some(rest) = trimmed.strip_prefix("mqtt://") {
        (rest, false)
    } else {
        (trimmed, false)
    };
    let env_tls = env_bool("TRUST_DIT_MQTT_TLS");
    let tls = scheme_tls || env_tls;
    let (host, port) = match remainder.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() => {
            let port = port
                .parse::<u16>()
                .map_err(|error| format!("invalid MQTT broker port '{port}': {error}"))?;
            (host.to_string(), port)
        }
        _ => (remainder.to_string(), if tls { 8883 } else { 1883 }),
    };
    if host.is_empty() {
        return Err("MQTT broker host must not be empty".to_string());
    }
    Ok(MqttEndpoint { host, port, tls })
}

fn mqtt_publish_cycle(
    cycle: usize,
    endpoint: &MqttEndpoint,
    credentials: Option<&(String, String)>,
    topic: &str,
) -> Result<Value, String> {
    let client_id = format!("trust-dit-{}-{cycle}", std::process::id());
    let mut options = MqttOptions::new(&client_id, &endpoint.host, endpoint.port);
    options.set_clean_session(true);
    options.set_keep_alive(Duration::from_secs(5));
    if let Some((username, password)) = credentials {
        options.set_credentials(username, password);
    }
    if endpoint.tls {
        options.set_transport(Transport::Tls(TlsConfiguration::NativeConnector(
            build_tls_connector()?,
        )));
    }

    let (client, mut connection) = Client::new(options, 16);
    let payload = format!("trust-device-in-loop-cycle-{cycle}");
    client
        .publish(topic, QoS::AtMostOnce, false, payload.as_bytes())
        .map_err(|error| format!("queue MQTT publish: {error}"))?;

    let mut connack = false;
    let mut publish_sent = false;
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline && !(connack && publish_sent) {
        match connection.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                events.push(format!("{event:?}"));
                match event {
                    Event::Incoming(Packet::ConnAck(_)) => connack = true,
                    Event::Outgoing(Outgoing::Publish(_)) => publish_sent = true,
                    _ => {}
                }
            }
            Ok(Err(error)) => return Err(format!("MQTT event loop error: {error}")),
            Err(error) => events.push(format!("timeout_or_closed:{error:?}")),
        }
    }
    if !connack {
        return Err(format!("MQTT cycle {cycle} did not receive CONNACK"));
    }
    if !publish_sent {
        return Err(format!("MQTT cycle {cycle} did not send PUBLISH"));
    }

    client
        .disconnect()
        .map_err(|error| format!("queue MQTT disconnect: {error}"))?;
    let mut disconnect_sent = false;
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !disconnect_sent {
        match connection.recv_timeout(Duration::from_millis(500)) {
            Ok(Ok(event)) => {
                events.push(format!("{event:?}"));
                if matches!(event, Event::Outgoing(Outgoing::Disconnect)) {
                    disconnect_sent = true;
                }
            }
            Ok(Err(error)) => {
                return Err(format!("MQTT event loop error after disconnect: {error}"))
            }
            Err(error) => events.push(format!("timeout_or_closed:{error:?}")),
        }
    }
    if !disconnect_sent {
        return Err(format!("MQTT cycle {cycle} did not send DISCONNECT"));
    }

    Ok(json!({
        "cycle": cycle,
        "client_id": client_id,
        "connack_received": connack,
        "publish_sent": publish_sent,
        "disconnect_sent": disconnect_sent,
        "events": events,
    }))
}

fn build_tls_connector() -> Result<TlsConnector, String> {
    let ca_path = env_text("TRUST_DIT_MQTT_TLS_CA_PATH")
        .ok_or_else(|| "TRUST_DIT_MQTT_TLS_CA_PATH is required for MQTT TLS".to_string())?;
    let ca = fs::read(&ca_path)
        .map_err(|error| format!("read TRUST_DIT_MQTT_TLS_CA_PATH '{ca_path}': {error}"))?;
    let ca = Certificate::from_pem(&ca)
        .map_err(|error| format!("parse MQTT TLS CA PEM '{ca_path}': {error}"))?;
    let mut builder = TlsConnector::builder();
    builder.add_root_certificate(ca);
    match (
        env_text("TRUST_DIT_MQTT_TLS_CLIENT_CERT_PATH"),
        env_text("TRUST_DIT_MQTT_TLS_CLIENT_KEY_PATH"),
    ) {
        (Some(cert_path), Some(key_path)) => {
            let cert = fs::read(&cert_path).map_err(|error| {
                format!("read TRUST_DIT_MQTT_TLS_CLIENT_CERT_PATH '{cert_path}': {error}")
            })?;
            let key = fs::read(&key_path).map_err(|error| {
                format!("read TRUST_DIT_MQTT_TLS_CLIENT_KEY_PATH '{key_path}': {error}")
            })?;
            let identity = Identity::from_pkcs8(&cert, &key)
                .map_err(|error| format!("parse MQTT mTLS identity: {error}"))?;
            builder.identity(identity);
        }
        (None, None) => {}
        _ => {
            return Err(
                "TRUST_DIT_MQTT_TLS_CLIENT_CERT_PATH and TRUST_DIT_MQTT_TLS_CLIENT_KEY_PATH must be set together"
                    .to_string(),
            );
        }
    }
    if let Some(alpn) = env_text("TRUST_DIT_MQTT_TLS_ALPN") {
        let protocols = alpn
            .split(',')
            .map(str::trim)
            .filter(|entry| !entry.is_empty())
            .collect::<Vec<_>>();
        builder.request_alpns(&protocols);
    }
    builder
        .build()
        .map_err(|error| format!("build MQTT TLS connector: {error}"))
}

fn skip_artifact(protocol: &str, file_name: &str, missing: &[&str], reason: &str) {
    let value = json!({
        "schema_version": SCHEMA_VERSION,
        "protocol": protocol,
        "status": "skipped",
        "timestamp_ms": now_ms(),
        "reason": reason,
        "missing": missing,
        "require_hardware": require_hardware(),
    });
    write_artifact(file_name, value);
    eprintln!("{protocol} device-in-the-loop skipped: {reason}; missing {missing:?}");
    if require_hardware() {
        panic!(
            "{protocol} device-in-the-loop required hardware but skipped: {reason}; see {}",
            artifact_path(file_name).display()
        );
    }
}

fn fail_artifact(protocol: &str, file_name: &str, reason: &str, detail: Value) -> ! {
    write_artifact(
        file_name,
        json!({
            "schema_version": SCHEMA_VERSION,
            "protocol": protocol,
            "status": "fail",
            "timestamp_ms": now_ms(),
            "reason": reason,
            "detail": detail,
        }),
    );
    panic!(
        "{protocol} device-in-the-loop failed: {reason}; see {}",
        artifact_path(file_name).display()
    );
}

fn write_artifact(file_name: &str, value: Value) {
    let path = artifact_path(file_name);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)
            .unwrap_or_else(|error| panic!("create artifact dir {}: {error}", parent.display()));
    }
    let text = serde_json::to_string_pretty(&value).expect("serialize device-in-loop artifact");
    fs::write(&path, format!("{text}\n"))
        .unwrap_or_else(|error| panic!("write artifact {}: {error}", path.display()));
}

fn artifact_path(file_name: &str) -> PathBuf {
    artifact_dir().join(file_name)
}

fn artifact_dir() -> PathBuf {
    env::var_os("TRUST_DIT_ARTIFACT_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| Path::new("target/device-in-the-loop").to_path_buf())
}

fn env_text(name: &str) -> Option<String> {
    env::var(name)
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
}

fn require_hardware() -> bool {
    env_bool("TRUST_DIT_REQUIRE_HARDWARE")
}

fn env_bool(name: &str) -> bool {
    env_text(name).is_some_and(|value| {
        matches!(
            value.to_ascii_lowercase().as_str(),
            "1" | "true" | "yes" | "y" | "on"
        )
    })
}

fn now_ms() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(target_os = "linux")]
fn current_rss_kib() -> Option<u64> {
    let status = fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let value = line.strip_prefix("VmRSS:")?.trim();
        value.split_whitespace().next()?.parse::<u64>().ok()
    })
}

#[cfg(not(target_os = "linux"))]
fn current_rss_kib() -> Option<u64> {
    None
}

fn max_optional(left: Option<u64>, right: Option<u64>) -> Option<u64> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.max(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
