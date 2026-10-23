use std::io::Write;
use std::process::{Command, Stdio};

#[test]
fn plant_serve_exchanges_one_response_per_request_until_eof() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system time after epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-runtime-plant-command-{}-{stamp}",
        std::process::id()
    ));
    std::fs::create_dir_all(&root).expect("create plant command fixture");
    std::fs::write(
        root.join("simulation.toml"),
        "[simulation]\nenabled = true\n",
    )
    .expect("write simulation configuration");

    let mut child = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args(["plant", "serve", "--project"])
    .arg(&root)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .expect("start stopped-project plant session");
    {
        let stdin = child.stdin.as_mut().expect("plant session stdin");
        writeln!(
            stdin,
            "{}",
            serde_json::json!({
                "id": 1,
                "type": "plant.capabilities",
                "params": { "contract": "trust.plant", "schema_version": 1 }
            })
        )
        .unwrap();
        writeln!(
            stdin,
            "{}",
            serde_json::json!({
                "id": 2,
                "type": "plant.read",
                "params": { "contract": "trust.plant", "schema_version": 1 }
            })
        )
        .unwrap();
    }
    drop(child.stdin.take());
    let output = child
        .wait_with_output()
        .expect("wait for plant session EOF");
    assert!(
        output.status.success(),
        "plant session failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let responses = String::from_utf8(output.stdout)
        .expect("UTF-8 plant responses")
        .lines()
        .map(|line| serde_json::from_str::<serde_json::Value>(line).expect("JSON response line"))
        .collect::<Vec<_>>();
    assert_eq!(responses.len(), 2);
    assert_eq!(responses[0].get("id"), Some(&serde_json::json!(1)));
    assert_eq!(responses[0].get("ok"), Some(&serde_json::json!(true)));
    assert_eq!(responses[1].get("id"), Some(&serde_json::json!(2)));
    assert_eq!(responses[1].get("ok"), Some(&serde_json::json!(true)));
    assert!(responses[1]
        .pointer("/result/configuration_revision")
        .is_some());
    std::fs::remove_dir_all(root).expect("remove plant command fixture");
}
