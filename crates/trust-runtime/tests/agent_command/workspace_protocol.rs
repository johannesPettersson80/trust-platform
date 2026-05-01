#[test]
fn agent_serve_supports_describe_write_and_read_roundtrip() {
    let project = unique_temp_dir("agent-workspace");
    let source_text = "PROGRAM Main\nEND_PROGRAM\n";
    fs::create_dir_all(&project).expect("create workspace root");

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

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "agent.describe",
        })
    )
    .expect("write describe request");
    let describe = read_response(&mut reader);
    assert_eq!(describe["result"]["framing"], json!("jsonl"));
    assert_json_absolute_path_eq(&describe["result"]["workspace_root"], &project);
    assert!(
        describe["result"]["methods"]
            .as_array()
            .expect("methods array")
            .iter()
            .any(|item| item == "lsp.ast_similarity"),
        "expected lsp.ast_similarity in agent.describe"
    );
    assert!(
        describe["result"]["methods"]
            .as_array()
            .expect("methods array")
            .iter()
            .any(|item| item == "harness.execute"),
        "expected harness.execute in agent.describe"
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "workspace.write",
            "params": {
                "path": "src/main.st",
                "text": source_text,
            },
        })
    )
    .expect("write workspace.write request");
    let write_response = read_response(&mut reader);
    assert_json_relative_path_eq(&write_response["result"]["path"], "src/main.st");
    assert_eq!(
        write_response["result"]["bytes_written"],
        json!(source_text.len())
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 3,
            "method": "workspace.read",
            "params": {
                "path": "src/main.st",
            },
        })
    )
    .expect("write workspace.read request");
    let read_response = read_response(&mut reader);
    assert_json_relative_path_eq(&read_response["result"]["path"], "src/main.st");
    assert_eq!(read_response["result"]["text"], json!(source_text));

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
fn trust_runtime_agent_alias_forwards_to_trust_dev() {
    let project = unique_temp_dir("agent-forward");
    fs::create_dir_all(&project).expect("create workspace root");

    let mut child = trust_runtime_command_with_dev_alias()
        .args(["agent", "serve", "--project"])
        .arg(&project)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-runtime agent forwarding alias");

    let mut stdin = child.stdin.take().expect("agent stdin");
    let stdout = child.stdout.take().expect("agent stdout");
    let mut reader = BufReader::new(stdout);
    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "agent.describe"
        }),
    );
    let response = read_response(&mut reader);
    assert_eq!(response["result"]["transport"], json!("stdio"));
    drop(stdin);

    let output = child.wait_with_output().expect("wait for agent process");
    assert!(
        output.status.success(),
        "agent forwarding alias should exit cleanly.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("trust-runtime agent serve") && stderr.contains("trust-dev agent serve"),
        "forwarding alias should print deprecation warning, got: {stderr}"
    );

    let _ = fs::remove_dir_all(project);
}

#[test]
fn agent_serve_reports_method_and_path_errors_with_stable_codes() {
    let project = unique_temp_dir("agent-errors");
    fs::create_dir_all(&project).expect("create workspace root");

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

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 11,
            "method": "workspace.missing",
        })
    )
    .expect("write missing-method request");
    let missing_method = read_response(&mut reader);
    assert_eq!(missing_method["error"]["code"], json!(-32601));

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 12,
            "method": "workspace.write",
            "params": {
                "path": "../escape.st",
                "text": "PROGRAM Main\nEND_PROGRAM\n",
            },
        })
    )
    .expect("write path-escape request");
    let path_escape = read_response(&mut reader);
    assert_eq!(path_escape["error"]["code"], json!(-32001));
    assert_eq!(
        path_escape["error"]["data"]["kind"],
        json!("path_outside_workspace")
    );

    writeln!(
        stdin,
        "{}",
        json!({
            "jsonrpc": "2.0",
            "id": 13,
            "method": "workspace.read",
        })
    )
    .expect("write missing-params request");
    let invalid_params = read_response(&mut reader);
    assert_eq!(invalid_params["error"]["code"], json!(-32602));

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

