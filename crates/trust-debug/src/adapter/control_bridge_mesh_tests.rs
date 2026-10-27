use super::*;
use trust_runtime::scheduler::ResourceCommand;

/// Specification 43 (Debug-session boundary): a debug session publishes no runtime mesh catalogue, so a mesh-catalogue
/// request that reaches the simulator's resource is answered with an explicit unavailable result. Catches a request
/// left unanswered and a catalogue that a debug session advertises. It needs the toolchain of specification 44 §4.6 at
/// `TRUST_TOOLCHAIN`.
#[test]
fn debug_mesh_catalogue_returns_an_explicit_unavailable_result() {
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let root = std::env::temp_dir().join(format!("debug-mesh-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(root.join("src")).unwrap();
    std::fs::write(
        root.join("src/Main.st"),
        "PROGRAM Main\nVAR\n    Level : REAL;\nEND_VAR\nEND_PROGRAM\n",
    )
    .unwrap();
    let (_session, simulator) = start_simulator(
        &root,
        &root.join("src/Main.st"),
        ControlEndpoint::Tcp("127.0.0.1:0".parse().unwrap()),
        None,
    );
    let (reply, response) = std::sync::mpsc::channel();
    simulator
        .control_server()
        .state()
        .resource
        .send_command(ResourceCommand::MeshCatalogue {
            names: vec!["Main.Level".to_owned()],
            respond_to: reply,
        })
        .unwrap();
    let result = response.recv_timeout(std::time::Duration::from_secs(1));
    simulator.stop();
    let _ = std::fs::remove_dir_all(root);
    assert!(
        matches!(result, Ok(Err(ref message)) if message.contains("unavailable in debug control")),
        "debug bridge must answer unsupported catalogue requests explicitly: {result:?}"
    );
}
