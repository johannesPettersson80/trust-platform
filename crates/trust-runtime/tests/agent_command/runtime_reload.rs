#[test]
fn agent_serve_runtime_reload_rebuilds_and_reloads_a_live_runtime() {
    let project = copy_fixture("green");
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind mock control listener");
    let port = listener.local_addr().expect("listener addr").port();
    let (request_tx, request_rx) = mpsc::sync_channel(1);
    let source_false = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;
    let source_true = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    patch_control_endpoint(&project, port);
    fs::write(project.join("src").join("main.st"), source_false).expect("write initial source");

    let build_output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["build", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        build_output.status.success(),
        "initial build failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let server = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept control connection");
        let mut reader = BufReader::new(stream.try_clone().expect("clone control stream"));
        let mut line = String::new();
        reader.read_line(&mut line).expect("read control request");
        let request = serde_json::from_str::<JsonValue>(line.trim()).expect("decode request");
        request_tx.send(request).expect("send request to test");
        writeln!(
            stream,
            "{}",
            json!({
                "id": 1,
                "ok": true,
                "result": { "status": "reloaded" }
            })
        )
        .expect("write control response");
        stream.flush().expect("flush control response");
    });

    fs::write(project.join("src").join("main.st"), source_true).expect("write updated source");

    let mut child = trust_dev_command()
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-dev agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);
    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 41,
            "method": "runtime.reload",
        }),
    );
    let reload = read_response(&mut reader);
    assert_eq!(reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(
        reload["result"]["reload"]["result"]["status"],
        json!("reloaded")
    );
    let request = request_rx
        .recv_timeout(Duration::from_secs(5))
        .expect("receive control request");
    assert_eq!(request["type"], json!("bytecode.reload"));
    assert!(
        request["params"]["bytes"]
            .as_str()
            .is_some_and(|bytes| !bytes.is_empty()),
        "expected bytecode payload in control request"
    );
    drop(stdin);
    let agent_output = child.wait_with_output().expect("wait for agent process");
    assert!(
        agent_output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&agent_output.stdout),
        String::from_utf8_lossy(&agent_output.stderr)
    );
    server.join().expect("join mock control server");
    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_blocks_on_diagnostics() {
    let project = copy_fixture("green");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n",
    )
    .expect("write broken source");

    let mut child = trust_dev_command()
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-dev agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 51,
            "method": "runtime.compile_reload",
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("skipped"));
    assert_eq!(
        compile_reload["result"]["runtimeMessage"],
        json!("Build and reload blocked by diagnostics.")
    );
    assert!(
        compile_reload["result"]["errors"]
            .as_u64()
            .expect("errors count")
            >= 1
    );
    assert_eq!(compile_reload["result"]["build"], JsonValue::Null);
    assert_eq!(compile_reload["result"]["reload"], JsonValue::Null);
    assert!(
        compile_reload["result"]["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|item| {
                item["message"]
                    .as_str()
                    .is_some_and(|message| message.contains("UnknownSymbol"))
            }),
        "expected unresolved symbol diagnostic"
    );

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_reports_reload_failure() {
    let project = copy_fixture("green");

    let mut child = trust_dev_command()
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-dev agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 52,
            "method": "runtime.compile_reload",
            "params": {
                "endpoint": "tcp://127.0.0.1:1"
            },
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("error"));
    assert!(
        compile_reload["result"]["runtimeMessage"]
            .as_str()
            .is_some_and(|message| message.contains("Reload failed")),
        "expected reload failure message"
    );
    assert_eq!(compile_reload["result"]["errors"], json!(0));
    assert_eq!(compile_reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(compile_reload["result"]["reload"], JsonValue::Null);

    drop(stdin);
    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_runtime_compile_reload_rebuilds_and_reloads_a_live_runtime() {
    let project = copy_fixture("green");
    let token = "trust-ci-token";
    let source_false = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := FALSE;
END_PROGRAM
"#;
    let source_true = r#"
PROGRAM Main
VAR
    q AT %QX0.0 : BOOL;
END_VAR
q := TRUE;
END_PROGRAM
"#;

    fs::write(project.join("src").join("main.st"), source_false).expect("write initial source");

    let build_output = Command::new(env!("CARGO_BIN_EXE_trust-runtime"))
        .args(["build", "--project"])
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        build_output.status.success(),
        "initial build failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let (mut runtime, control_endpoint) = spawn_runtime_with_retry(&project, token);
    wait_for_output_bool(
        &control_endpoint,
        token,
        "%QX0.0",
        false,
        Duration::from_secs(20),
    );

    fs::write(project.join("src").join("main.st"), source_true).expect("write updated source");

    let mut child = trust_dev_command()
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-dev agent serve");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);
    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 53,
            "method": "runtime.compile_reload",
        }),
    );
    let compile_reload = read_response(&mut reader);
    assert_eq!(compile_reload["result"]["runtimeStatus"], json!("ok"));
    assert_eq!(
        compile_reload["result"]["runtimeMessage"],
        json!("Runtime reload succeeded.")
    );
    assert_eq!(compile_reload["result"]["build"]["status"], json!("ok"));
    assert_eq!(
        compile_reload["result"]["reload"]["result"]["status"],
        json!("reloaded")
    );
    drop(stdin);
    let agent_output = child.wait_with_output().expect("wait for agent process");
    assert!(
        agent_output.status.success(),
        "agent serve should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&agent_output.stdout),
        String::from_utf8_lossy(&agent_output.stderr)
    );

    wait_for_output_bool(
        &control_endpoint,
        token,
        "%QX0.0",
        true,
        Duration::from_secs(20),
    );

    let _ = control_request(&control_endpoint, token, "shutdown", None);
    let _ = runtime.kill();
    let _ = runtime.wait();
    let _ = fs::remove_dir_all(project);
}
