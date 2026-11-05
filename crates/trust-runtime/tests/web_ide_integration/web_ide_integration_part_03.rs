use super::*;

#[test]
fn web_ide_latency_and_resource_budget_contract() {
    let _alone = MEASURING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let project = make_project("budget");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let (_, caps) = request_json("GET", &format!("{base}/api/ide/capabilities"), None, &[]);
    let max_file_bytes = caps
        .get("result")
        .and_then(|v| v.get("limits"))
        .and_then(|v| v.get("max_file_bytes"))
        .and_then(Value::as_u64)
        .expect("max_file_bytes");

    let (_, session) = request_json(
        "POST",
        &format!("{base}/api/ide/session"),
        Some(json!({ "role": "editor" })),
        &[],
    );
    let token = session
        .get("result")
        .and_then(|v| v.get("token"))
        .and_then(Value::as_str)
        .expect("session token");

    let (_, opened) = request_json(
        "GET",
        &format!("{base}/api/ide/file?path=main.st"),
        None,
        &[("X-Trust-Ide-Session", token)],
    );
    let mut version = opened
        .get("result")
        .and_then(|v| v.get("version"))
        .and_then(Value::as_u64)
        .expect("initial version");

    let runs: u32 = 60;
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    for idx in 0..runs {
        let started = Instant::now();
        let (status, result) = request_json(
            "POST",
            &format!("{base}/api/ide/file"),
            Some(json!({
                "path": "main.st",
                "expected_version": version,
                "content": format!(
                    "PROGRAM Main\\nVAR\\nCounter : INT := {};\\nEND_VAR\\nEND_PROGRAM\\n",
                    idx
                )
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        assert_eq!(status, 200);
        version = result
            .get("result")
            .and_then(|v| v.get("version"))
            .and_then(Value::as_u64)
            .expect("next version");
    }

    let avg = total / runs;
    assert!(
        max < Duration::from_millis(250),
        "max latency {:?} exceeded budget",
        max
    );
    assert!(
        avg < Duration::from_millis(50),
        "avg latency {:?} exceeded budget",
        avg
    );

    let too_large = "X".repeat(max_file_bytes as usize + 1);
    let (status, body) = request_json(
        "POST",
        &format!("{base}/api/ide/file"),
        Some(json!({
            "path": "main.st",
            "expected_version": version,
            "content": too_large
        })),
        &[("X-Trust-Ide-Session", token)],
    );
    assert_eq!(status, 413);
    assert!(body
        .get("error")
        .and_then(Value::as_str)
        .is_some_and(|message| message.contains("exceeds limit")));

    let _ = std::fs::remove_dir_all(project);
}
