use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use serde_json::json;

use super::*;

static NEXT_PROJECT_ID: AtomicU64 = AtomicU64::new(0);

fn project(label: &str) -> PathBuf {
    let id = NEXT_PROJECT_ID.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-comm-apply-instance-id-{}-{label}-{id}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create comm.apply project");
    root
}

fn request(value: serde_json::Value) -> CommApplyRequest {
    serde_json::from_value(value).expect("decode comm.apply request")
}

fn modbus_params(address: &str, unit_id: u64) -> serde_json::Value {
    json!({
        "address": address,
        "unit_id": unit_id,
        "input_start": 0,
        "output_start": 0,
        "timeout_ms": 500,
        "on_error": "warn",
    })
}

#[test]
fn save_returns_the_written_instance_id() {
    let root = project("written-id");
    fs::write(
        root.join("io.toml"),
        concat!(
            "[io]\n",
            "safe_state = [{ address = \"%QW2\", value = \"4242\" }]\n",
            "\n",
            "[[io.drivers]]\n",
            "name = \"modbus-tcp\"\n",
            "\n",
            "[io.drivers.params]\n",
            "address = \"127.0.0.1:1502\"\n",
            "unit_id = 2\n",
            "input_start = 5\n",
            "output_start = 7\n",
            "timeout_ms = 750\n",
            "on_error = \"warn\"\n",
        ),
    )
    .expect("write io.toml");

    // spec 11 §6.8.1a: adding a second Modbus TCP driver writes it at index 1.
    let added = apply_request_with_project_root(
        Some(root.as_path()),
        request(json!({
            "protocol": "modbus_tcp",
            "action": "add",
            "params": modbus_params("127.0.0.1:1503", 3),
        })),
    );
    // A wrong implementation that echoed request.instance_id would answer None here.
    assert!(added.applied, "add must apply: {added:?}");
    assert_eq!(
        added.instance_id.as_deref(),
        Some("modbus_tcp:1"),
        "an add must answer the project-local id it wrote"
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    let first = text.find("127.0.0.1:1502").expect("first driver address");
    let second = text.find("127.0.0.1:1503").expect("second driver address");
    assert!(
        first < second,
        "the added driver must be written at index 1: {text}"
    );

    // Editing the first driver keeps its index 0.
    let edited = apply_request_with_project_root(
        Some(root.as_path()),
        request(json!({
            "protocol": "modbus_tcp",
            "action": "edit",
            "instance_id": "modbus_tcp:0",
            "params": modbus_params("127.0.0.1:1504", 2),
        })),
    );
    // A wrong implementation that reused index 1 or echoed the request id fails here.
    assert!(edited.applied, "edit must apply: {edited:?}");
    assert_eq!(
        edited.instance_id.as_deref(),
        Some("modbus_tcp:0"),
        "an edit must answer the project-local id it wrote"
    );
    let text = fs::read_to_string(root.join("io.toml")).expect("read io.toml");
    assert!(text.contains("127.0.0.1:1504"), "{text}");
    assert!(
        !text.contains("127.0.0.1:1502"),
        "the edited driver must be replaced: {text}"
    );

    fs::remove_dir_all(&root).ok();
}
