//! Which launch's simulator a document's inline values reach (specification 14).

use super::*;

fn control(endpoint: &str) -> SimulatorControl {
    SimulatorControl {
        endpoint: endpoint.to_owned(),
        auth_token: format!("{endpoint}-token"),
    }
}

#[test]
fn a_document_reaches_the_simulator_of_its_innermost_workspace_folder() {
    // Catches: a document in a nested project reaching the outer folder's simulator, or a document
    // in another folder reaching a simulator that is not its own.
    let controls = SimulatorControls::default();
    controls.record(Some(PathBuf::from("/work")), control("outer"));
    controls.record(Some(PathBuf::from("/work/plant")), control("plant"));
    controls.record(Some(PathBuf::from("/other")), control("other"));

    let reached = |path: &str| controls.for_document(Path::new(path)).map(|c| c.endpoint);
    assert_eq!(reached("/work/plant/src/main.st").as_deref(), Some("plant"));
    assert_eq!(reached("/work/line/src/main.st").as_deref(), Some("outer"));
    assert_eq!(reached("/other/main.st").as_deref(), Some("other"));
    assert_eq!(
        reached("/work-two/main.st"),
        None,
        "a folder named like the start of another folder's name does not hold its documents"
    );
}

#[test]
fn a_document_outside_every_folder_reaches_the_simulator_named_without_one() {
    // Catches: a launch without a workspace folder, whose simulator no document could reach.
    let controls = SimulatorControls::default();
    controls.record(None, control("default"));
    controls.record(Some(PathBuf::from("/work/plant")), control("plant"));

    let reached = |path: &str| controls.for_document(Path::new(path)).map(|c| c.endpoint);
    assert_eq!(reached("/elsewhere/main.st").as_deref(), Some("default"));
    assert_eq!(reached("/work/plant/main.st").as_deref(), Some("plant"));
}

#[test]
fn a_new_simulator_of_a_folder_replaces_the_earlier_one() {
    // Catches: inline values that keep reaching the first simulator of a folder after the server
    // named another one for it.
    let controls = SimulatorControls::default();
    controls.record(Some(PathBuf::from("/work/plant")), control("first"));
    controls.record(Some(PathBuf::from("/work/plant")), control("second"));

    let reached = controls
        .for_document(Path::new("/work/plant/main.st"))
        .expect("a simulator");
    assert_eq!(reached, control("second"));
}

#[test]
fn an_endpoint_named_for_another_folder_is_passed_over_and_its_own_is_not() {
    // Catches: a new simulator of one folder named the port that this server named for another
    // (two workspaces whose hashes share a Windows port, the first's simulator not yet running),
    // and a folder that passes over its own earlier port.
    let controls = SimulatorControls::default();
    controls.record(
        Some(PathBuf::from("/work/plant")),
        control("tcp://127.0.0.1:22699"),
    );

    let named = |folder: &str, endpoint: &str| {
        controls.named_for_another_folder(Some(Path::new(folder)), endpoint)
    };
    assert!(named("/work/line", "tcp://127.0.0.1:22699"));
    assert!(!named("/work/plant", "tcp://127.0.0.1:22699"));
    assert!(!named("/work/line", "tcp://127.0.0.1:22700"));
}
