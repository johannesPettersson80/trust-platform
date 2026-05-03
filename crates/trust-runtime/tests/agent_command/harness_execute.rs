#[test]
fn agent_serve_supports_harness_execute_for_pou_and_project_fixtures() {
    let project = unique_temp_dir("agent-harness-execute");
    fs::create_dir_all(project.join("src")).expect("create src directory");
    fs::write(
        project.join("runtime.toml"),
        r#"[bundle]
version = 1

[resource]
name = "Res"
cycle_interval_ms = 100

[runtime.control]
endpoint = "tcp://127.0.0.1:0"
auth_token = "trust-ci-token"

[runtime.web]
enabled = false
listen = "127.0.0.1:0"

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
        project.join("src").join("main.st"),
        r#"
PROGRAM Main
VAR
    input AT %IX0.0 : BOOL;
    output AT %QX0.0 : BOOL;
END_VAR
output := input;
END_PROGRAM
"#,
    )
    .expect("write project source");

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
            "id": 320,
            "method": "harness.execute",
            "params": {
                "inline_sources": [
                    { "text": r#"
PROGRAM Main
VAR
    in1 : BOOL;
    ton_fb : TON;
    q : BOOL;
    et : TIME;
END_VAR
ton_fb(IN := in1, PT := T#30MS, Q => q, ET => et);
END_PROGRAM
"# }
                ],
                "steps": [
                    {
                        "op": "set_input",
                        "name": "in1",
                        "value": { "type": "BOOL", "value": true }
                    },
                    {
                        "op": "run_until",
                        "name": "q",
                        "equals": { "type": "BOOL", "value": true },
                        "dt_ms": 10,
                        "max_cycles": 5
                    }
                ],
                "assertions": [
                    {
                        "kind": "output_equals",
                        "name": "q",
                        "equals": { "type": "BOOL", "value": true }
                    },
                    {
                        "kind": "output_equals",
                        "name": "et",
                        "equals": { "type": "TIME", "nanos": 30_000_000 }
                    }
                ],
                "watch": ["q", "et"]
            },
        }),
    );
    let pou = read_response(&mut reader);
    assert_eq!(pou["result"]["status"], json!("pass"));
    assert_eq!(pou["result"]["passed"], json!(true));
    assert_eq!(pou["result"]["stepsRun"], json!(2));
    assert_eq!(pou["result"]["assertions"]["total"], json!(2));
    assert_eq!(pou["result"]["assertions"]["passed"], json!(2));
    assert_eq!(pou["result"]["assertions"]["failed"], json!(0));
    assert_eq!(pou["result"]["watchSnapshot"]["cycleCount"], json!(4));
    assert_eq!(
        pou["result"]["watchSnapshot"]["values"]["q"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": true}})
    );
    assert_eq!(
        pou["result"]["watchSnapshot"]["values"]["et"],
        json!({"status": "ok", "value": {"type": "TIME", "nanos": 30_000_000}})
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 321,
            "method": "harness.execute",
            "params": {
                "steps": [
                    {
                        "op": "set_direct_input",
                        "address": "%IX0.0",
                        "value": { "type": "BOOL", "value": true }
                    },
                    {
                        "op": "cycle",
                        "count": 1
                    }
                ],
                "assertions": [
                    {
                        "kind": "direct_output_equals",
                        "address": "%QX0.0",
                        "equals": { "type": "BOOL", "value": true }
                    }
                ],
                "watch": ["output"]
            },
        }),
    );
    let system = read_response(&mut reader);
    assert_eq!(system["result"]["status"], json!("pass"));
    assert_eq!(system["result"]["passed"], json!(true));
    assert_eq!(system["result"]["sourceCount"], json!(1));
    assert_eq!(system["result"]["stepsRun"], json!(2));
    assert_eq!(system["result"]["assertions"]["total"], json!(1));
    assert_eq!(system["result"]["assertions"]["passed"], json!(1));
    assert_eq!(system["result"]["watchSnapshot"]["cycleCount"], json!(2));
    assert_eq!(
        system["result"]["watchSnapshot"]["values"]["output"],
        json!({"status": "ok", "value": {"type": "BOOL", "value": true}})
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
