use std::process::Command;

/// A control endpoint where nothing answers, for as long as the returned holder lives: a port held
/// without listening (specification 22), which no other socket can take.
fn unreachable_endpoint() -> (trust_test_support::RefusedPort, String) {
    let held = trust_test_support::refused_loopback_port();
    let endpoint = format!("tcp://127.0.0.1:{}", held.port());
    (held, endpoint)
}

#[test]
fn trust_runtime_ui_no_input_smoke() {
    let (_held, endpoint) = unreachable_endpoint();
    let output = Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-runtime")
            .expect("Cargo must provide trust-runtime binary while executing integration tests"),
    )
    .args([
        "ui",
        "--endpoint",
        &endpoint,
        "--no-input",
        "--refresh",
        "10",
    ])
    .output()
    .expect("run trust-runtime ui --no-input");

    assert!(
        !output.status.success(),
        "expected connection failure smoke"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("Error:"),
        "expected formatted runtime error, stderr was: {stderr}"
    );
}
