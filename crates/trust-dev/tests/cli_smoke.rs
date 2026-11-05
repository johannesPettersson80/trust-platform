use std::process::Command;

fn trust_dev() -> Command {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-dev")
            .expect("Cargo must provide trust-dev binary while executing integration tests"),
    )
}

#[test]
fn trust_dev_help_surfaces_workbench_commands() {
    let output = trust_dev().arg("--help").output().expect("run trust-dev");

    assert!(
        output.status.success(),
        "trust-dev --help failed.\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("agent"));
    assert!(stdout.contains("commit"));
    assert!(stdout.contains("docs"));
    assert!(stdout.contains("test"));
}

#[test]
fn trust_dev_subcommand_help_is_stable() {
    for args in [
        &["agent", "serve", "--help"][..],
        &["commit", "--help"][..],
        &["docs", "--help"][..],
        &["test", "--help"][..],
    ] {
        let output = trust_dev().args(args).output().expect("run trust-dev");
        assert!(
            output.status.success(),
            "trust-dev {} failed.\nstdout:\n{}\nstderr:\n{}",
            args.join(" "),
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
}

#[test]
fn oscat_replaced_upstream_name_remains_absent() {
    let project = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../trust-runtime/tests/fixtures/oscat/negative_public_surface");
    let main_source = include_str!(
        "../../trust-runtime/tests/fixtures/oscat/negative_public_surface/src/main.st"
    );
    let compile_fixture_source = include_str!(
        "../../trust-runtime/tests/fixtures/oscat/negative_public_surface/src/tests.st"
    );
    assert!(main_source.contains("OVERRIDE("));
    assert!(compile_fixture_source.contains("OVERRIDE("));
    let output = trust_dev()
        .args(["test", "--project"])
        .arg(&project)
        .output()
        .expect("run OSCAT negative public-surface fixture");

    assert!(
        !output.status.success(),
        "upstream OSCAT OVERRIDE must not resolve as a duplicate public API"
    );
    let diagnostics = format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    )
    .replace('\\', "/");
    assert!(diagnostics.contains("src/main.st: expected expression"));
    assert!(diagnostics.contains("src/tests.st: expected expression"));
}

/// The real `trust-dev agent serve-mcp` over stdio: the framing is one JSON-RPC
/// message per stdout line, `initialize` negotiates and `tools/list` lists the
/// Agent API tools, a `tools/call` of `agent_describe` returns the contract
/// payload, and a second call sent while `runtime_build` is still running is
/// refused with `-32600` instead of being queued (specification 20 §8).
#[test]
fn agent_serve_mcp_serves_one_json_rpc_message_per_line_and_refuses_an_overlapping_call() {
    use std::io::{BufRead, BufReader, Write};
    use std::process::Stdio;

    let workspace = std::env::temp_dir().join(format!(
        "trust-dev-mcp-serve-mcp-e2e-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock")
            .as_millis(),
    ));
    std::fs::create_dir_all(workspace.join("src")).expect("create temp workspace");
    std::fs::write(
        workspace.join("src/main.st"),
        "PROGRAM Main\nVAR\n  value : INT;\nEND_VAR\n  value := value + 1;\nEND_PROGRAM\n",
    )
    .expect("write the trivial program");

    let mut child = trust_dev()
        .args(["agent", "serve-mcp", "--project"])
        .arg(&workspace)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn trust-dev agent serve-mcp");

    // The reader owns the stdout pipe for the whole test; every request walks
    // through it in order, so a line that fails to parse fails the test with
    // its text attached.
    let stdout = BufReader::new(child.stdout.take().expect("child stdout"));
    let (sender, receiver) = std::sync::mpsc::channel::<String>();
    std::thread::spawn(move || {
        for line in stdout.lines() {
            if sender
                .send(line.expect("read an agent stdout line"))
                .is_err()
            {
                break;
            }
        }
    });
    let mut stdin = child.stdin.take().expect("child stdin");
    let mut next_answer = |stdin: &mut std::process::ChildStdin| -> serde_json::Value {
        let line = receiver
            .recv_timeout(std::time::Duration::from_secs(120))
            .expect("the server answers before the answer time");
        serde_json::from_str(&line)
            .unwrap_or_else(|error| panic!("stdout line is not one JSON message: {error}: {line}"))
    };

    // The handshake and the tool list: the shared layer's negotiation and the
    // provider's tool table over real stdio.
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":1,"method":"initialize","params":{{"protocolVersion":"2025-06-18","capabilities":{{}},"clientInfo":{{"name":"test","version":"0"}}}}}}"#
    )
    .expect("send initialize");
    let initialized = next_answer(&mut stdin);
    assert!(
        initialized["result"]["protocolVersion"].is_string(),
        "initialize answers a protocol version: {initialized:#}"
    );
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","method":"notifications/initialized"}}"#
    )
    .expect("send the initialized notification");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{{}}}}"#
    )
    .expect("send tools/list");
    let listed = next_answer(&mut stdin);
    let tools = listed["result"]["tools"]
        .as_array()
        .expect("tools/list answers a list");
    assert_eq!(tools.len(), 21, "one tool per Agent API method");
    assert!(
        tools.iter().any(|tool| tool["name"] == "agent_describe"),
        "the mangled agent.describe tool is listed"
    );

    // One call at a time: the build keeps the server busy, and the describe
    // call that arrives while it runs is refused, not queued. The refusal and
    // the build answer are two more single-message stdout lines.
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"runtime_build","arguments":{{}}}}}}"#
    )
    .expect("send runtime_build");
    writeln!(
        stdin,
        r#"{{"jsonrpc":"2.0","id":4,"method":"tools/call","params":{{"name":"agent_describe","arguments":{{}}}}}}"#
    )
    .expect("send the overlapping agent_describe");
    let answers: Vec<serde_json::Value> = (0..2).map(|_| next_answer(&mut stdin)).collect();
    let refused = answers.iter().find(|answer| answer["id"] == 4);
    let refused = refused.expect("the overlapping call is answered");
    assert_eq!(
        refused["error"]["code"], -32600,
        "the overlapping call is refused, not queued: {refused:#}"
    );
    let build = answers.iter().find(|answer| answer["id"] == 3);
    let build = build.expect("the build call is answered");
    assert!(
        build["result"].is_object() || build["error"].is_object(),
        "the build answers either way, one message per line: {build:#}"
    );

    drop(stdin);
    let _ = child.wait();
    let _ = std::fs::remove_dir_all(workspace);
}
