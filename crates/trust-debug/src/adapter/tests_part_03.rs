use super::*;

/// Specification 13, "Multi-Task Thread Model": the threads are the project's tasks in declaration
/// order, numbered from 1, and one more, `Background`, for the programs that run in no task, all
/// read from the declarations of the launch's project. Catches threads numbered in another order, a
/// task left out, and programs without a task given no thread. The three are instances of one
/// program type (IEC 61131-3 §6.8.2, Table 62 6a).
#[test]
fn dispatch_threads_maps_tasks() {
    let root = std::env::temp_dir().join(format!(
        "trust-debug-threads-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos())
    ));
    std::fs::create_dir_all(&root).unwrap();
    let program = root.join("main.st");
    std::fs::write(
        &program,
        r#"PROGRAM Main
END_PROGRAM

CONFIGURATION Config
RESOURCE Res ON PLC
    TASK FAST (INTERVAL := T#10ms, PRIORITY := 1);
    TASK SLOW (INTERVAL := T#20ms, PRIORITY := 2);
    PROGRAM P1 WITH FAST : Main;
    PROGRAM P2 WITH SLOW : Main;
    PROGRAM P3 : Main;
END_RESOURCE
END_CONFIGURATION
"#,
    )
    .unwrap();
    let mut session = DebugSession::new();
    session
        .launch_sources(&program.to_string_lossy())
        .expect("the project's sources");
    let mut adapter = DebugAdapter::new(session);

    let threads_req = Request::<serde_json::Value> {
        seq: 1,
        message_type: MessageType::Request,
        command: "threads".to_string(),
        arguments: None,
    };
    let threads_outcome = adapter.dispatch_request(threads_req);
    let threads_response: Response<ThreadsResponseBody> =
        serde_json::from_value(threads_outcome.responses[0].clone()).unwrap();
    let threads = threads_response.body.unwrap().threads;
    assert_eq!(threads.len(), 3);
    assert_eq!(threads[0].id, 1);
    assert_eq!(threads[0].name, "FAST");
    assert_eq!(threads[1].id, 2);
    assert_eq!(threads[1].name, "SLOW");
    assert_eq!(threads[2].id, 3);
    assert_eq!(threads[2].name, "Background");
    std::fs::remove_dir_all(&root).ok();
}
