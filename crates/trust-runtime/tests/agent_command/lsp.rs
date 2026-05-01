#[test]
fn agent_serve_supports_lsp_diagnostics_and_format_preview() {
    let project = unique_temp_dir("agent-lsp");
    fs::create_dir_all(project.join("src")).expect("create src directory");
    fs::write(
        project.join("src").join("main.st"),
        "PROGRAM Main\nEND_PROGRAM\n",
    )
    .expect("write seed source");

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
            "id": 28,
            "method": "lsp.diagnostics",
            "params": {
                "path": "src/main.st",
                "content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n",
            },
        }),
    );
    let diagnostics = read_response(&mut reader);
    assert_json_absolute_path_eq(
        &diagnostics["result"]["target"],
        &project.join("src/main.st"),
    );
    assert!(
        diagnostics["result"]["errors"]
            .as_u64()
            .expect("errors count")
            >= 1
    );
    assert_eq!(
        normalize_path_text(
            diagnostics["result"]["issues"][0]["path"]
                .as_str()
                .expect("diagnostic path")
        ),
        "src/main.st"
    );
    let unknown_symbol = diagnostics["result"]["issues"]
        .as_array()
        .expect("issues array")
        .iter()
        .find(|item| {
            item["message"]
                .as_str()
                .is_some_and(|message| message.contains("UnknownSymbol"))
        })
        .expect("UnknownSymbol diagnostic");
    assert_eq!(unknown_symbol["line"], json!(6));
    assert_eq!(unknown_symbol["column"], json!(11));
    assert_eq!(unknown_symbol["endLine"], json!(6));
    assert_eq!(unknown_symbol["endColumn"], json!(26));
    assert_eq!(unknown_symbol["span"]["start"], json!(51));
    assert_eq!(unknown_symbol["span"]["end"], json!(66));
    assert!(
        diagnostics["result"]["issues"]
            .as_array()
            .expect("issues array")
            .iter()
            .any(|item| item["severity"] == json!("error")),
        "expected at least one error diagnostic"
    );
    assert!(
        diagnostics["result"]["issues"]
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

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 29,
            "method": "lsp.format",
            "params": {
                "path": "src/main.st",
                "content": "PROGRAM Main\nVAR\nCounter:INT;\nEND_VAR\nIF Counter > 0 THEN\nCounter:=Counter+1;\nELSE\nCounter:=0;\nEND_IF\nEND_PROGRAM\n",
            },
        }),
    );
    let format = read_response(&mut reader);
    assert_json_relative_path_eq(&format["result"]["path"], "src/main.st");
    assert_eq!(format["result"]["changed"], json!(true));
    assert_eq!(
        format["result"]["content"],
        json!(
            "PROGRAM Main\n  VAR\n    Counter:INT;\n  END_VAR\n  IF Counter > 0 THEN\n    Counter:=Counter+1;\n  ELSE\n    Counter:=0;\n  END_IF\nEND_PROGRAM\n"
        )
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
fn agent_serve_supports_ast_canonicalize_and_similarity() {
    let project = unique_temp_dir("agent-ast");
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

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 280,
            "method": "lsp.ast_canonicalize",
            "params": {
                "content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := 1;\nEND_PROGRAM\n",
            },
        }),
    );
    let canonical = read_response(&mut reader);
    assert_eq!(
        canonical["result"]["algorithm"],
        json!("canonical_ast_jaccard_5gram_v1")
    );
    assert_eq!(canonical["result"]["gramSize"], json!(5));
    assert!(!canonical["result"]["fiveGrams"]
        .as_array()
        .expect("five grams array")
        .is_empty());

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 281,
            "method": "lsp.ast_similarity",
            "params": {
                "left_content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := 1;\nEND_PROGRAM\n",
                "right_content": "PROGRAM Demo\nVAR\nValue : INT;\nEND_VAR\nValue := 42;\nEND_PROGRAM\n",
            },
        }),
    );
    let similar = read_response(&mut reader);
    assert_eq!(similar["result"]["score"], json!(1.0));
    assert_eq!(similar["result"]["threshold070"], json!(true));
    assert_eq!(similar["result"]["threshold095"], json!(true));

    write_request(
        &mut stdin,
        json!({
            "jsonrpc": "2.0",
            "id": 282,
            "method": "lsp.ast_similarity",
            "params": {
                "left_content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nCounter := Counter + 1;\nEND_PROGRAM\n",
                "right_content": "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\nIF Counter > 0 THEN\nCounter := Counter + 1;\nEND_IF\nEND_PROGRAM\n",
            },
        }),
    );
    let dissimilar = read_response(&mut reader);
    assert_eq!(dissimilar["result"]["threshold070"], json!(false));
    assert_eq!(dissimilar["result"]["threshold095"], json!(false));
    assert!(dissimilar["result"]["score"].as_f64().expect("score") < 0.70);

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

