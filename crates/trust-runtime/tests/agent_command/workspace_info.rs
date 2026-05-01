#[test]
fn agent_serve_reports_workspace_project_info() {
    let project = unique_temp_dir("agent-project-info");
    let dependency = project.join("deps").join("lib-a");
    fs::create_dir_all(project.join("src")).expect("create src directory");
    fs::create_dir_all(dependency.join("src")).expect("create dependency src directory");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("write project source");
    fs::write(
        dependency.join("src").join("lib.st"),
        "FUNCTION DepDouble : INT\nVAR_INPUT x : INT; END_VAR\nDepDouble := x * 2;\nEND_FUNCTION\n",
    )
    .expect("write dependency source");
    fs::write(
        project.join("runtime.toml"),
        r#"[bundle]
version = 1

[resource]
name = "Res"
cycle_interval_ms = 100

[runtime.control]
endpoint = "tcp://127.0.0.1:0"
auth_token = "secret"

[runtime.log]
level = "info"

[runtime.retain]
mode = "none"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 5000
action = "halt"

[runtime.fault]
policy = "halt"
"#,
    )
    .expect("write runtime.toml");
    fs::write(
        project.join("io.toml"),
        "[io]\ndriver = \"simulated\"\nparams = {}\n",
    )
    .expect("write io.toml");
    fs::write(
        project.join("trust-lsp.toml"),
        r#"[project]
vendor_profile = "codesys"

[dependencies]
LibA = { path = "deps/lib-a", version = "1.0.0" }
"#,
    )
    .expect("write trust-lsp.toml");
    fs::write(
        dependency.join("trust-lsp.toml"),
        "[package]\nversion = \"1.0.0\"\n",
    )
    .expect("write dependency manifest");

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
            "id": 30,
            "method": "workspace.project_info",
        }),
    );
    let project_info = read_response(&mut reader);
    assert_json_absolute_path_eq(&project_info["result"]["project"], &project);
    assert_eq!(project_info["result"]["sourceCount"], json!(2));
    assert_eq!(
        project_info["result"]["resolvedDependencies"],
        json!(["LibA"])
    );
    assert_eq!(
        project_info["result"]["lsp"]["vendorProfile"],
        json!("codesys")
    );
    assert_eq!(
        project_info["result"]["runtime"]["controlEndpoint"],
        json!("tcp://127.0.0.1:0")
    );
    assert_eq!(
        project_info["result"]["runtime"]["hasControlToken"],
        json!(true)
    );
    assert_eq!(
        project_info["result"]["io"]["drivers"],
        json!(["simulated"])
    );
    assert_eq!(
        project_info["result"]["files"]["trustLspToml"]["exists"],
        json!(true)
    );
    assert!(
        project_info["result"]["sources"]
            .as_array()
            .expect("sources array")
            .iter()
            .any(|value| value == "src/main.st"),
        "expected project source in sources list"
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
fn agent_serve_reports_run_until_timeout_with_stable_code() {
    let project = unique_temp_dir("agent-run-until-timeout");
    let harness_program = r#"
PROGRAM Main
VAR
    flag : BOOL;
END_VAR
END_PROGRAM
"#;

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
            "id": 31,
            "method": "harness.load",
            "params": {
                "inline_sources": [
                    { "text": harness_program }
                ]
            },
        }),
    );
    let load = read_response(&mut reader);
    assert_eq!(load["result"]["source_count"], json!(1));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 32,
            "method": "harness.run_until",
            "params": {
                "name": "flag",
                "equals": { "type": "BOOL", "value": true },
                "max_cycles": 2
            },
        }),
    );
    let timeout = read_response(&mut reader);
    assert_eq!(timeout["error"]["code"], json!(-32004));
    assert_eq!(timeout["error"]["data"]["name"], json!("flag"));
    assert_eq!(timeout["error"]["data"]["max_cycles"], json!(2));

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

