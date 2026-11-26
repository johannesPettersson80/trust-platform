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
    controls.name_and_record(Some(PathBuf::from("/work")), |_| control("outer"));
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |_| control("plant"));
    controls.name_and_record(Some(PathBuf::from("/other")), |_| control("other"));

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
    controls.name_and_record(None, |_| control("default"));
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |_| control("plant"));

    let reached = |path: &str| controls.for_document(Path::new(path)).map(|c| c.endpoint);
    assert_eq!(reached("/elsewhere/main.st").as_deref(), Some("default"));
    assert_eq!(reached("/work/plant/main.st").as_deref(), Some("plant"));
}

#[test]
fn a_new_simulator_of_a_folder_replaces_the_earlier_one() {
    // Catches: inline values that keep reaching the first simulator of a folder after the server
    // named another one for it.
    let controls = SimulatorControls::default();
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |_| control("first"));
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |_| control("second"));

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
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |_| {
        control("tcp://127.0.0.1:22699")
    });
    let mut seen = None;
    controls.name_and_record(Some(PathBuf::from("/work/line")), |named_elsewhere| {
        seen = Some((
            named_elsewhere("tcp://127.0.0.1:22699"),
            named_elsewhere("tcp://127.0.0.1:22700"),
        ));
        control("tcp://127.0.0.1:22700")
    });
    assert_eq!(
        seen,
        Some((true, false)),
        "another folder's endpoint, then a free one"
    );
    let mut own = None;
    controls.name_and_record(Some(PathBuf::from("/work/plant")), |named_elsewhere| {
        own = Some(named_elsewhere("tcp://127.0.0.1:22699"));
        control("tcp://127.0.0.1:22699")
    });
    assert_eq!(
        own,
        Some(false),
        "a folder's own endpoint is not another folder's"
    );
}

#[test]
fn two_folders_asked_for_at_once_get_two_endpoints() {
    // Catches a choice and a record under separate locks: both folders would see the same
    // endpoint free before either keeps it (Codex's review of #839).
    let controls = std::sync::Arc::new(SimulatorControls::default());
    let pick = |named_elsewhere: &dyn Fn(&str) -> bool| {
        let endpoint = ["tcp://127.0.0.1:22699", "tcp://127.0.0.1:22700"]
            .into_iter()
            .find(|endpoint| !named_elsewhere(endpoint))
            .expect("a free endpoint");
        // A slow choice, as the port probe is, widens the window in which both could choose.
        std::thread::sleep(std::time::Duration::from_millis(50));
        control(endpoint)
    };
    let threads: Vec<_> = ["/work/one", "/work/two"]
        .into_iter()
        .map(|folder| {
            let controls = std::sync::Arc::clone(&controls);
            std::thread::spawn(move || {
                controls
                    .name_and_record(Some(PathBuf::from(folder)), pick)
                    .endpoint
            })
        })
        .collect();
    let mut endpoints: Vec<String> = threads
        .into_iter()
        .map(|thread| thread.join().expect("a thread"))
        .collect();
    endpoints.sort();
    assert_eq!(
        endpoints,
        ["tcp://127.0.0.1:22699", "tcp://127.0.0.1:22700"]
    );
}

#[test]
fn a_windows_folder_holds_its_documents_in_any_case() {
    // Windows' file systems ignore case, so VS Code can name a folder `C:\Work\Plant` and one of
    // its documents `c:\work\plant\src\main.st` (Codex's review of #839); elsewhere case
    // matters. Catches a lookup that misses the folder's simulator on Windows, and two records of
    // one Windows folder asked for in two cases.
    let controls = SimulatorControls::default();
    controls.name_and_record(Some(PathBuf::from(r"C:\Work\Plant")), |_| control("first"));
    let reached = controls
        .for_document(Path::new(r"c:\work\plant\src\main.st"))
        .map(|control| control.endpoint);
    if cfg!(windows) {
        assert_eq!(reached.as_deref(), Some("first"));
        controls.name_and_record(Some(PathBuf::from(r"c:\work\plant")), |_| control("second"));
        let replaced = controls
            .for_document(Path::new(r"C:\WORK\PLANT\main.st"))
            .map(|control| control.endpoint);
        assert_eq!(
            replaced.as_deref(),
            Some("second"),
            "one record of one folder"
        );
    } else {
        assert_eq!(reached, None, "case matters outside Windows");
    }
}
