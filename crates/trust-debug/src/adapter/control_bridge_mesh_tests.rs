use super::*;

/// Specification 43: debug sessions explicitly refuse mesh publication queries.
#[test]
fn debug_mesh_catalogue_returns_an_explicit_unavailable_result() {
    let (commands, receiver) = std::sync::mpsc::channel();
    let drain = spawn_command_drain(
        receiver,
        DebugControl::new(),
        Arc::new(Mutex::new(trust_runtime::Runtime::new())),
        Arc::new(Mutex::new(None)),
    );
    let (reply, response) = std::sync::mpsc::channel();
    commands
        .send(ResourceCommand::MeshCatalogue {
            names: vec!["Main.Level".to_owned()],
            respond_to: reply,
        })
        .unwrap();
    let result = response.recv_timeout(std::time::Duration::from_secs(1));
    drop(commands);
    drain.join().unwrap();
    assert!(
        matches!(result, Ok(Err(ref message)) if message.contains("unavailable in debug control")),
        "debug bridge must answer unsupported catalogue requests explicitly: {result:?}"
    );
}
