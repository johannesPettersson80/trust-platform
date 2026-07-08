use super::*;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

fn project_dir(name: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "trust-runtime-web-ide-{name}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create project dir");
    path
}

fn project_dir_in_current_dir(name: &str) -> PathBuf {
    let path = std::env::current_dir()
        .expect("current dir")
        .join("target")
        .join("trust-runtime-web-ide")
        .join(format!("{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).expect("create project dir");
    path
}

fn write_source(project: &Path, rel: &str, content: &str) {
    let path = project.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).expect("create source parent");
    }
    std::fs::write(path, content).expect("write source");
}

#[test]
fn auth_and_session_lifecycle_contract() {
    let project = project_dir("session");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let clock = Arc::new(AtomicU64::new(10_000));
    let state = WebIdeState::with_clock(
        Some(project.clone()),
        Arc::new({
            let clock = clock.clone();
            move || clock.load(Ordering::SeqCst)
        }),
    );

    let err = state.list_sources("missing").expect_err("missing session");
    assert_eq!(err.kind(), IdeErrorKind::Unauthorized);

    let session = state
        .create_session(IdeRole::Viewer)
        .expect("create viewer session");
    let files = state
        .list_sources(&session.token)
        .expect("list files with session");
    assert_eq!(files, vec!["main.st".to_string()]);

    clock.store(10_000 + SESSION_TTL_SECS + 1, Ordering::SeqCst);
    let expired_err = state
        .open_source(&session.token, "main.st")
        .expect_err("session should be expired");
    assert_eq!(expired_err.kind(), IdeErrorKind::Unauthorized);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn session_activity_renews_ttl_and_idle_expiry_still_applies() {
    let project = project_dir("sliding-ttl");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let clock = Arc::new(AtomicU64::new(20_000));
    let state = WebIdeState::with_clock(
        Some(project.clone()),
        Arc::new({
            let clock = clock.clone();
            move || clock.load(Ordering::SeqCst)
        }),
    );

    let session = state
        .create_session(IdeRole::Viewer)
        .expect("create viewer session");

    clock.store(20_000 + (SESSION_TTL_SECS / 2), Ordering::SeqCst);
    let _ = state
        .list_sources(&session.token)
        .expect("active request should renew ttl");

    clock.store(20_000 + SESSION_TTL_SECS + 5, Ordering::SeqCst);
    let _ = state
        .open_source(&session.token, "main.st")
        .expect("session should still be valid after renewal");

    clock.store(20_000 + (2 * SESSION_TTL_SECS) + 10, Ordering::SeqCst);
    let expired = state
        .list_sources(&session.token)
        .expect_err("idle session should expire");
    assert_eq!(expired.kind(), IdeErrorKind::Unauthorized);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn session_limit_evicts_oldest_inactive_session() {
    let clock = Arc::new(AtomicU64::new(30_000));
    let state = WebIdeState::with_clock(
        None,
        Arc::new({
            let clock = clock.clone();
            move || clock.load(Ordering::SeqCst)
        }),
    );

    let mut tokens = Vec::new();
    for _ in 0..MAX_SESSIONS {
        let session = state
            .create_session(IdeRole::Viewer)
            .expect("create initial session");
        tokens.push(session.token);
    }

    clock.store(30_100, Ordering::SeqCst);
    let keep_alive_token = tokens.last().expect("last token").to_string();
    let _ = state
        .project_selection(&keep_alive_token)
        .expect("renew one active session");

    let replacement = state
        .create_session(IdeRole::Viewer)
        .expect("create replacement session");

    let replacement_ok = state
        .project_selection(&replacement.token)
        .expect("replacement session remains valid");
    assert!(replacement_ok.active_project.is_none());

    let renewed_ok = state
        .project_selection(&keep_alive_token)
        .expect("renewed session remains valid");
    assert!(renewed_ok.active_project.is_none());

    let mut evicted_count = 0;
    for token in &tokens {
        if token == &keep_alive_token {
            continue;
        }
        if state.project_selection(token).is_err() {
            evicted_count += 1;
        }
    }
    assert_eq!(
        evicted_count, 1,
        "exactly one stale session should be evicted"
    );
}

#[test]
fn collaborative_conflict_detected_with_expected_version() {
    let project = project_dir("conflict");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let s1 = state
        .create_session(IdeRole::Editor)
        .expect("create editor session 1");
    let s2 = state
        .create_session(IdeRole::Editor)
        .expect("create editor session 2");

    let doc1 = state
        .open_source(&s1.token, "main.st")
        .expect("open from s1");
    let doc2 = state
        .open_source(&s2.token, "main.st")
        .expect("open from s2");
    assert_eq!(doc1.version, doc2.version);

    let write1 = state
        .apply_source(
            &s1.token,
            "main.st",
            doc1.version,
            "PROGRAM Main\nVAR\nA : INT;\nEND_VAR\nEND_PROGRAM\n".to_string(),
            true,
        )
        .expect("apply first edit");
    assert!(write1.version > doc1.version);

    let conflict = state
        .apply_source(
            &s2.token,
            "main.st",
            doc2.version,
            "PROGRAM Main\nVAR\nB : INT;\nEND_VAR\nEND_PROGRAM\n".to_string(),
            true,
        )
        .expect_err("stale write must conflict");
    assert_eq!(conflict.kind(), IdeErrorKind::Conflict);
    assert_eq!(conflict.current_version(), Some(write1.version));

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn latency_and_resource_budgets_are_enforced() {
    let project = project_dir("budget");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");
    let mut snapshot = state
        .open_source(&session.token, "main.st")
        .expect("open source");

    let runs = 80_u32;
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;

    for idx in 0..runs {
        let started = Instant::now();
        let result = state
            .apply_source(
                &session.token,
                "main.st",
                snapshot.version,
                format!(
                    "PROGRAM Main\\nVAR\\nA : INT := {};\\nEND_VAR\\nEND_PROGRAM\\n",
                    idx
                ),
                true,
            )
            .expect("apply edit within budget");
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        snapshot.version = result.version;
    }

    let avg = total / runs;
    assert!(
        max < Duration::from_millis(250),
        "max apply latency {:?} exceeded budget",
        max
    );
    assert!(
        avg < Duration::from_millis(40),
        "avg apply latency {:?} exceeded budget",
        avg
    );

    let too_large = "X".repeat(MAX_FILE_BYTES + 1);
    let err = state
        .apply_source(&session.token, "main.st", snapshot.version, too_large, true)
        .expect_err("oversized payload should fail");
    assert_eq!(err.kind(), IdeErrorKind::TooLarge);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn diagnostics_hover_and_completion_contracts_are_exposed() {
    let project = project_dir("analysis");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");

    let diagnostics = state
            .diagnostics(
                &session.token,
                "main.st",
                Some(
                    "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := UnknownSymbol + 1;\nEND_PROGRAM\n"
                        .to_string(),
                ),
            )
            .expect("diagnostics");
    assert!(
        diagnostics
            .iter()
            .any(|item| item.message.contains("UnknownSymbol")),
        "expected unresolved symbol diagnostic"
    );

    let hover = state
            .hover(
                &session.token,
                "main.st",
                Some(
                    "PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCounter := Counter + 1;\nEND_PROGRAM\n"
                        .to_string(),
                ),
                Position {
                    line: 5,
                    character: 2,
                },
            )
            .expect("hover");
    assert!(hover.is_some(), "hover payload should be available");

    let completion = state
        .completion(
            &session.token,
            "main.st",
            Some("PRO\nPROGRAM Main\nEND_PROGRAM\n".to_string()),
            Position {
                line: 0,
                character: 3,
            },
            Some(20),
        )
        .expect("completion");
    assert!(
        completion.iter().any(|item| item.label == "PROGRAM"),
        "completion should include PROGRAM"
    );

    let in_scope_completion = state
        .completion(
            &session.token,
            "main.st",
            Some("PROGRAM Main\nVAR\nCounter : INT;\nEND_VAR\n\nCoun\nEND_PROGRAM\n".to_string()),
            Position {
                line: 5,
                character: 4,
            },
            Some(20),
        )
        .expect("in-scope completion");
    assert!(
        in_scope_completion
            .iter()
            .take(3)
            .any(|item| item.label == "Counter"),
        "expected in-scope symbol in top-3 suggestions"
    );

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn format_structured_text_document_indents_common_blocks() {
    let input = "PROGRAM Main\nVAR\nCounter:INT;\nEND_VAR\nIF Counter > 0 THEN\nCounter:=Counter+1;\nELSE\nCounter:=0;\nEND_IF\nEND_PROGRAM\n";
    let expected = "PROGRAM Main\n  VAR\n    Counter:INT;\n  END_VAR\n  IF Counter > 0 THEN\n    Counter:=Counter+1;\n  ELSE\n    Counter:=0;\n  END_IF\nEND_PROGRAM\n";
    assert_eq!(format_structured_text_document(input), expected);
}

#[test]
fn format_source_endpoint_returns_formatted_content_without_write() {
    let project = project_dir("format-source");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");

    let result = state
        .format_source(
            &session.token,
            "main.st",
            Some("PROGRAM Main\nVAR\nA:INT;\nEND_VAR\nEND_PROGRAM\n".to_string()),
        )
        .expect("format source");
    assert_eq!(result.path, "main.st");
    assert!(result.changed);
    assert!(result.content.contains("  VAR"));
    assert!(result.content.contains("    A:INT;"));

    let disk = std::fs::read_to_string(project.join("main.st")).expect("read disk source");
    assert_eq!(disk, "PROGRAM Main\nEND_PROGRAM\n");

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn health_snapshot_reports_active_state() {
    let project = project_dir("health");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let viewer = state
        .create_session(IdeRole::Viewer)
        .expect("create viewer session");
    let editor = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");
    let _ = state
        .open_source(&editor.token, "main.st")
        .expect("open source");

    let health = state.health(&viewer.token).expect("health");
    assert_eq!(health.active_sessions, 2);
    assert_eq!(health.editor_sessions, 1);
    assert_eq!(health.tracked_documents, 1);
    assert_eq!(health.open_document_handles, 1);
    assert_eq!(health.frontend_telemetry.bootstrap_failures, 0);
    assert_eq!(health.frontend_telemetry.analysis_timeouts, 0);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn frontend_telemetry_is_aggregated_in_health_snapshot() {
    let project = project_dir("frontend-telemetry");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let s1 = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");
    let s2 = state
        .create_session(IdeRole::Viewer)
        .expect("create viewer session");

    state
        .record_frontend_telemetry(
            &s1.token,
            WebIdeFrontendTelemetry {
                bootstrap_failures: 1,
                analysis_timeouts: 2,
                worker_restarts: 0,
                autosave_failures: 3,
            },
        )
        .expect("record telemetry session 1");
    state
        .record_frontend_telemetry(
            &s2.token,
            WebIdeFrontendTelemetry {
                bootstrap_failures: 4,
                analysis_timeouts: 1,
                worker_restarts: 2,
                autosave_failures: 0,
            },
        )
        .expect("record telemetry session 2");

    let health = state.health(&s1.token).expect("health");
    assert_eq!(health.frontend_telemetry.bootstrap_failures, 5);
    assert_eq!(health.frontend_telemetry.analysis_timeouts, 3);
    assert_eq!(health.frontend_telemetry.worker_restarts, 2);
    assert_eq!(health.frontend_telemetry.autosave_failures, 3);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn workspace_search_respects_include_and_exclude_globs() {
    let project = project_dir("workspace-search-globs");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");
    write_source(
        &project,
        "types.st",
        "TYPE\nMyType : STRUCT\nvalue : INT;\nEND_STRUCT;\nEND_TYPE\n",
    );

    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");

    let scoped = state
        .workspace_search(&session.token, "MyType", Some("types.st"), None, 50)
        .expect("search with include glob");
    assert!(!scoped.is_empty());
    assert!(scoped.iter().all(|hit| hit.path == "types.st"));

    let excluded = state
        .workspace_search(&session.token, "MyType", None, Some("types.st"), 50)
        .expect("search with exclude glob");
    assert!(excluded.iter().all(|hit| hit.path != "types.st"));

    let invalid = state
        .workspace_search(&session.token, "Main", Some("["), None, 10)
        .expect_err("invalid include glob should fail");
    assert_eq!(invalid.kind(), IdeErrorKind::InvalidInput);

    let _ = std::fs::remove_dir_all(project);
}

#[test]
fn project_selection_and_switch_flow_updates_active_root() {
    let project_a = project_dir_in_current_dir("project-switch-a");
    let project_b = project_dir_in_current_dir("project-switch-b");
    write_source(&project_a, "main.st", "PROGRAM Main\nEND_PROGRAM\n");
    write_source(&project_b, "alt.st", "PROGRAM Alt\nEND_PROGRAM\n");

    let state = WebIdeState::new(None);
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");

    let initial = state
        .project_selection(&session.token)
        .expect("project selection");
    assert!(initial.active_project.is_none());

    let switched = state
        .set_active_project(&session.token, project_b.to_string_lossy().as_ref())
        .expect("set active project");
    assert!(switched
        .active_project
        .as_ref()
        .is_some_and(|path| path.contains("project-switch-b")));

    let files = state
        .list_sources(&session.token)
        .expect("list switched project files");
    assert_eq!(files, vec!["alt.st".to_string()]);

    let _ = std::fs::remove_dir_all(project_a);
    let _ = std::fs::remove_dir_all(project_b);
}

#[test]
fn fs_audit_log_tracks_mutating_operations() {
    let project = project_dir("fs-audit");
    write_source(&project, "main.st", "PROGRAM Main\nEND_PROGRAM\n");

    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");

    let _ = state
        .create_entry(&session.token, "folder_a", true, None, true)
        .expect("create directory");
    let _ = state
        .create_entry(
            &session.token,
            "folder_a/extra.st",
            false,
            Some("PROGRAM Extra\nEND_PROGRAM\n".to_string()),
            true,
        )
        .expect("create file");
    let _ = state
        .rename_entry(
            &session.token,
            "folder_a/extra.st",
            "folder_a/renamed_extra.st",
            true,
        )
        .expect("rename file");
    let _ = state
        .delete_entry(&session.token, "folder_a/renamed_extra.st", true)
        .expect("delete file");

    let audit = state
        .fs_audit(&session.token, 20)
        .expect("read fs audit events");
    assert!(audit.len() >= 4);
    let health = state.health(&session.token).expect("health");
    assert!(health.fs_mutation_events >= 4);

    let _ = std::fs::remove_dir_all(project);
}
