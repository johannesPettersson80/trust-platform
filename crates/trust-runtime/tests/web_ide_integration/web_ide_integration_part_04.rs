use super::*;

#[test]
fn web_ide_reference_performance_gates_contract() {
    if std::env::consts::ARCH != "x86_64" {
        eprintln!(
            "skipping x86_64 reference-environment performance budget on {}",
            std::env::consts::ARCH
        );
        return;
    }

    let project = make_project("perf-gates");
    let state = control_state(source_fixture(), ControlMode::Debug, None);
    let base = start_test_server(state, project.clone(), WebAuthMode::Local);

    let mut boot_samples = Vec::new();
    for _ in 0..8 {
        let started = Instant::now();
        let _ = request_json("GET", &format!("{base}/api/ide/capabilities"), None, &[]);
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
        let _ = request_json(
            "GET",
            &format!("{base}/api/ide/files"),
            None,
            &[("X-Trust-Ide-Session", token)],
        );
        let _ = request_json(
            "GET",
            &format!("{base}/api/ide/file?path=main.st"),
            None,
            &[("X-Trust-Ide-Session", token)],
        );
        boot_samples.push(started.elapsed());
    }

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

    let doc_text =
        "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := Counter + 1;\nEND_PROGRAM\n";

    let mut completion_samples = Vec::new();
    let mut hover_samples = Vec::new();
    let mut diagnostics_samples = Vec::new();
    let mut search_samples = Vec::new();

    for _ in 0..35 {
        let started = Instant::now();
        let (status, _) = request_json(
            "POST",
            &format!("{base}/api/ide/completion"),
            Some(json!({
                "path": "main.st",
                "content": doc_text,
                "position": { "line": 5, "character": 7 },
                "limit": 30
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        assert_eq!(status, 200);
        completion_samples.push(started.elapsed());

        let started = Instant::now();
        let (status, _) = request_json(
            "POST",
            &format!("{base}/api/ide/hover"),
            Some(json!({
                "path": "main.st",
                "content": doc_text,
                "position": { "line": 5, "character": 2 }
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        assert_eq!(status, 200);
        hover_samples.push(started.elapsed());

        let started = Instant::now();
        let (status, _) = request_json(
            "POST",
            &format!("{base}/api/ide/diagnostics"),
            Some(json!({
                "path": "main.st",
                "content": doc_text
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        assert_eq!(status, 200);
        diagnostics_samples.push(started.elapsed());

        let started = Instant::now();
        let (status, _) = request_json(
            "GET",
            &format!("{base}/api/ide/search?q=Counter&include=**/*.st&limit=40"),
            None,
            &[("X-Trust-Ide-Session", token)],
        );
        assert_eq!(status, 200);
        search_samples.push(started.elapsed());
    }

    let two_k_line_content = {
        let mut lines = String::new();
        lines.push_str("PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n");
        for idx in 0..2000 {
            lines.push_str(&format!("Counter := Counter + {idx};\n"));
        }
        lines.push_str("END_PROGRAM\n");
        lines
    };
    let mut typing_freeze_max = Duration::ZERO;
    for _ in 0..25 {
        let started = Instant::now();
        let (status, _) = request_json(
            "POST",
            &format!("{base}/api/ide/completion"),
            Some(json!({
                "path": "main.st",
                "content": two_k_line_content,
                "position": { "line": 1200, "character": 12 },
                "limit": 20
            })),
            &[("X-Trust-Ide-Session", token)],
        );
        assert_eq!(status, 200);
        typing_freeze_max = typing_freeze_max.max(started.elapsed());
    }

    assert!(
        p95(&boot_samples) <= Duration::from_millis(2500),
        "boot-to-ready p95 exceeded 2.5s budget: {:?}",
        p95(&boot_samples)
    );
    assert!(
        p95(&completion_samples) <= Duration::from_millis(150),
        "completion p95 exceeded 150ms budget: {:?}",
        p95(&completion_samples)
    );
    assert!(
        p95(&hover_samples) <= Duration::from_millis(150),
        "hover p95 exceeded 150ms budget: {:?}",
        p95(&hover_samples)
    );
    assert!(
        p95(&diagnostics_samples) <= Duration::from_millis(300),
        "diagnostics p95 exceeded 300ms budget: {:?}",
        p95(&diagnostics_samples)
    );
    assert!(
        p95(&search_samples) <= Duration::from_millis(400),
        "workspace search p95 exceeded 400ms budget: {:?}",
        p95(&search_samples)
    );
    assert!(
        typing_freeze_max <= Duration::from_millis(800),
        "typing freeze max exceeded 800ms budget: {:?}",
        typing_freeze_max
    );

    let _ = std::fs::remove_dir_all(project);
}
