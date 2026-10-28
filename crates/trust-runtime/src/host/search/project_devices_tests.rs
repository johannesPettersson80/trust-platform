//! Tests of the reader of a project's devices (specification 11, "Compare with the project", "The
//! project's devices"). Every expected value is a literal worked out from the specification.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::{json, Value};
use smol_str::SmolStr;

use super::*;
use crate::bundle_template::render_runtime_toml;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A new project folder holding a valid `runtime.toml` and nothing else.
fn project() -> PathBuf {
    let folder = std::env::temp_dir().join(format!(
        "trust-project-devices-{}-{}",
        std::process::id(),
        COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&folder).expect("create the project folder");
    fs::write(
        folder.join("runtime.toml"),
        render_runtime_toml(&SmolStr::new("cell"), 100),
    )
    .expect("write runtime.toml");
    folder
}

fn write(folder: &Path, file: &str, text: &str) {
    let path = folder.join(file);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("create the folder of a project file");
    }
    fs::write(path, text).expect("write a project file");
}

fn facts(devices: &[ConfiguredDevice]) -> Vec<Value> {
    devices
        .iter()
        .map(|device| Value::Object(device.facts()))
        .collect()
}

const ADS_TOML: &str = r#"
[[connections]]
name = "line1"
target_net_id = "5.23.91.12.1.1"
host = "192.168.10.5"
ams_port = 851
transport = "plain"
insecure_transport = true

[[connections.points]]
symbol = "MAIN.Temperature"
var = "line1_temp"
type = "REAL"
"#;

const OPCUA_CLIENT_TOML: &str = r#"
[[connections]]
name = "line1"
endpoint_url = "opc.tcp://127.0.0.1:4840/trust"
security_policy = "none"
security_mode = "none"
auth = "anonymous"
trust_server_certificate = false

[[connections.points]]
var = "conveyor_speed"
node_id = "ns=2;s=MAIN.conveyor_speed"
type = "REAL"
access = "read"
"#;

#[test]
fn a_project_names_the_devices_of_its_enabled_drivers() {
    // Specification 11, "The project's devices": the enabled `modbus-tcp`, `ethercat` and `mqtt` drivers of
    // `io.toml` name devices, under any alias the driver registry accepts (`mqtt-tcp`, `ecat`), in the order
    // of the file; a driver with `enabled = false` and the other drivers (`gpio`) name none. A wrong
    // implementation lists the disabled driver, misses an alias, or lists the GPIO driver.
    let folder = project();
    write(
        &folder,
        "io.toml",
        r#"
[[io.drivers]]
name = "modbus-tcp"
params = { address = "10.0.0.9:502", unit_id = 2, product_code = "DRV-1" }

[[io.drivers]]
name = "modbus_tcp"
enabled = false
params = { address = "10.0.0.10:502" }

[[io.drivers]]
name = "mqtt-tcp"
params = { broker = "10.0.0.5:1883", allow_insecure_remote = true }

[[io.drivers]]
name = "ecat"
params = { adapter = "eth1", modules = [{ model = "EK1100", slot = 0 }, { model = "EL2008", slot = 2 }] }

[[io.drivers]]
name = "gpio"
params = { chip = "gpiochip0" }
"#,
    );

    let devices = load(&folder).expect("the project's devices");

    assert_eq!(
        facts(&devices),
        vec![
            json!({ "address": "10.0.0.9:502", "unit_id": 2, "product_code": "DRV-1" }),
            json!({ "address": "10.0.0.5:1883" }),
            json!({ "adapter": "eth1", "position": 0, "model": "EK1100" }),
            json!({ "adapter": "eth1", "position": 2, "model": "EL2008" }),
        ]
    );
    fs::remove_dir_all(folder).ok();
}

#[test]
fn ads_and_opcua_client_connections_count_only_when_runtime_toml_enables_them() {
    // Specification 11, "The project's devices": the ADS file and the OPC UA client file that `runtime.toml`
    // names with `config_path` name devices only while `[runtime.ads]` and `[runtime.opcua_client]` enable
    // them. A wrong implementation reads a file that the project does not run, or ignores `config_path`.
    let folder = project();
    write(&folder, "plc/ads.toml", ADS_TOML);
    write(&folder, "opcua_client.toml", OPCUA_CLIENT_TOML);
    assert_eq!(load(&folder).expect("the devices"), Vec::new());

    let runtime = fs::read_to_string(folder.join("runtime.toml")).expect("read runtime.toml");
    write(
        &folder,
        "runtime.toml",
        &format!("{runtime}\n[runtime.ads]\nenabled = true\nconfig_path = \"plc/ads.toml\"\n"),
    );
    assert_eq!(
        facts(&load(&folder).expect("the devices")),
        vec![json!({ "ams_net_id": "5.23.91.12.1.1", "address": "192.168.10.5" })]
    );

    let enabled = fs::read_to_string(folder.join("runtime.toml"))
        .expect("read runtime.toml")
        .replace(
            "[runtime.opcua_client]\nenabled = false",
            "[runtime.opcua_client]\nenabled = true",
        );
    assert!(enabled.contains("[runtime.opcua_client]\nenabled = true"));
    write(&folder, "runtime.toml", &enabled);
    assert_eq!(
        facts(&load(&folder).expect("the devices")),
        vec![
            json!({ "ams_net_id": "5.23.91.12.1.1", "address": "192.168.10.5" }),
            json!({ "address": "127.0.0.1:4840", "security": "none / none" }),
        ]
    );
    fs::remove_dir_all(folder).ok();
}

#[test]
fn a_folder_without_runtime_toml_is_not_a_project() {
    // Specification 11, "Request" and "The project's devices": the devices come from a project folder. A
    // folder without `runtime.toml` is refused with its name, not read as a project without devices, so that
    // a mistyped folder does not make every device look unused.
    let folder = std::env::temp_dir().join(format!(
        "trust-project-devices-empty-{}",
        std::process::id()
    ));
    fs::create_dir_all(&folder).expect("create the folder");

    let error = load(&folder).expect_err("not a project");

    assert!(
        error.contains(&folder.display().to_string()) && error.contains("has no runtime.toml"),
        "{error}"
    );
    fs::remove_dir_all(folder).ok();
}

#[test]
fn a_file_that_names_no_device_the_way_the_runtime_reads_it_is_refused_with_its_name() {
    // Specification 11, "Request" (`compare_project_unreadable`): a driver whose address is not an IP address
    // and a port, and an enabled ADS file that is missing, are refused with the file and the reason. A wrong
    // implementation skips the driver, and compares against a project that is not the project.
    let folder = project();
    write(
        &folder,
        "io.toml",
        "[[io.drivers]]\nname = \"modbus-tcp\"\nparams = { address = \"plc.local:502\" }\n",
    );
    let error = load(&folder).expect_err("a driver that names no device");
    assert!(
        error.contains("io.toml")
            && error.contains("driver 1")
            && error.contains("not an IP address and a port"),
        "{error}"
    );

    fs::remove_file(folder.join("io.toml")).expect("remove io.toml");
    let runtime = fs::read_to_string(folder.join("runtime.toml")).expect("read runtime.toml");
    write(
        &folder,
        "runtime.toml",
        &format!("{runtime}\n[runtime.ads]\nenabled = true\n"),
    );
    let error = load(&folder).expect_err("an ADS file that is missing");
    assert!(error.contains("ads.toml"), "{error}");
    fs::remove_dir_all(folder).ok();
}
