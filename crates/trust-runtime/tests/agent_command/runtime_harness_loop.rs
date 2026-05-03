#[test]
fn agent_serve_supports_runtime_project_commands_and_harness_loop() {
    let project = copy_fixture("green");
    let harness_program = r#"
PROGRAM Main
VAR
    in1 : BOOL;
    ton_fb : TON;
    q : BOOL;
    et : TIME;
END_VAR
ton_fb(IN := in1, PT := T#30MS, Q => q, ET => et);
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
            "id": 21,
            "method": "runtime.build",
        }),
    );
    let build = read_response(&mut reader);
    assert_eq!(build["result"]["status"], json!("ok"));
    assert_eq!(build["result"]["command"], json!("build"));
    assert_eq!(build["result"]["source_count"], json!(2));
    assert!(
        project.join("program.stbc").is_file(),
        "build should write program.stbc"
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 22,
            "method": "runtime.validate",
        }),
    );
    let validate = read_response(&mut reader);
    assert_eq!(validate["result"]["status"], json!("ok"));
    assert_eq!(validate["result"]["command"], json!("validate"));
    assert_json_absolute_path_eq(&validate["result"]["project"], &project);

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 23,
            "method": "runtime.test",
        }),
    );
    let tests = read_response(&mut reader);
    assert_eq!(tests["result"]["summary"]["passed"], json!(2));
    assert_eq!(tests["result"]["summary"]["failed"], json!(0));
    assert_eq!(tests["result"]["summary"]["errors"], json!(0));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 24,
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
            "id": 25,
            "method": "harness.set_input",
            "params": {
                "name": "in1",
                "value": { "type": "BOOL", "value": true }
            },
        }),
    );
    let set_input = read_response(&mut reader);
    assert_eq!(set_input["result"]["status"], json!("ok"));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 26,
            "method": "harness.run_until",
            "params": {
                "name": "q",
                "equals": { "type": "BOOL", "value": true },
                "dt_ms": 10,
                "max_cycles": 5,
                "watch": ["q", "et"]
            },
        }),
    );
    let run_until = read_response(&mut reader);
    assert_eq!(run_until["result"]["name"], json!("q"));
    assert_eq!(run_until["result"]["cycles_ran"], json!(3));
    assert_eq!(
        run_until["result"]["matched_value"],
        json!({"type": "BOOL", "value": true})
    );
    assert_eq!(
        run_until["result"]["values"]["et"],
        json!({"status": "ok", "value": {"type": "TIME", "nanos": 30_000_000}})
    );

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 27,
            "method": "harness.get_output",
            "params": {
                "name": "q"
            },
        }),
    );
    let get_output = read_response(&mut reader);
    assert_eq!(
        get_output["result"]["value"],
        json!({"type": "BOOL", "value": true})
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
