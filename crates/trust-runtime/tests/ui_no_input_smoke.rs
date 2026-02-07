use std::net::TcpListener;
use std::process::Command;

fn unreachable_endpoint() -> String {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind ephemeral loopback port");
    let addr = listener.local_addr().expect("read local addr");
    drop(listener);
    format!("tcp://{addr}")
}

#[test]
fn trust_runtime_ui_no_input_smoke() {
    let endpoint = unreachable_endpoint();
    let output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
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
