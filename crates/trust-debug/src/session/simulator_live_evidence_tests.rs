use super::*;
use trust_runtime::io::IoDriverHealth;
use trust_runtime::value::Duration;

/// A project folder with one program, its configuration and the given project files.
fn project(label: &str, files: &[(&str, &str)]) -> (std::path::PathBuf, std::path::PathBuf) {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos();
    let root = std::env::temp_dir().join(format!(
        "trust-debug-{label}-{}-{nanos}",
        std::process::id()
    ));
    let src = root.join("src");
    std::fs::create_dir_all(&src).expect("create project src");
    std::fs::write(
        src.join("main.st"),
        "PROGRAM Main\nVAR\n    Count : INT;\nEND_VAR\nCount := Count + 1;\nEND_PROGRAM\n",
    )
    .expect("write program");
    let config = src.join("config.st");
    std::fs::write(
        &config,
        "CONFIGURATION Config\nRESOURCE Cpu ON PLC\n    TASK MainTask (INTERVAL := T#100ms, PRIORITY := 1);\n    PROGRAM MainInstance WITH MainTask : Main;\nEND_RESOURCE\nEND_CONFIGURATION\n",
    )
    .expect("write configuration");
    for (name, text) in files {
        std::fs::write(root.join(name), text).expect("write project file");
    }
    (root, config)
}

fn loaded_session(root: &std::path::Path, config: &std::path::Path) -> DebugSession {
    let mut session = DebugSession::new(trust_runtime::RuntimeBuilder::new().build().unwrap());
    session.update_source_options(SourceOptionsUpdate {
        root: Some(root.to_string_lossy().to_string()),
        include_globs: None,
        exclude_globs: None,
        ignore_pragmas: None,
    });
    session
        .reload_program(Some(config.to_string_lossy().as_ref()))
        .expect("load the project");
    session
}

fn scan(session: &DebugSession, at_ms: i64) {
    let handle = session.runtime_handle();
    let mut runtime = handle.lock().unwrap();
    runtime.set_current_time(Duration::from_millis(at_ms));
    runtime.execute_cycle().expect("scan the program");
}

/// Specification 47 (Devices & Connections shows the simulator live): the simulator's control server reports its I/O
/// drivers' health in `fleet.topology`, as the runtime binary does. Catches the list the control server reads staying
/// empty, so the canvas drew the running simulator's I/O card as "Configured", and a reload (which replaces the
/// runtime) leaving the list behind.
#[test]
fn a_reloaded_simulator_reports_its_driver_health_to_the_control_list() {
    let (root, config) = project(
        "driver-health",
        &[("io.toml", "[io]\ndriver = \"simulated\"\n")],
    );
    let session = loaded_session(&root, &config);
    scan(&session, 100);
    let simulated = |session: &DebugSession| {
        session
            .io_health()
            .lock()
            .unwrap()
            .iter()
            .any(|status| status.name == "simulated" && status.health == IoDriverHealth::Ok)
    };
    assert!(
        simulated(&session),
        "the scan reports the simulated driver as healthy"
    );

    // A reload builds a new runtime; it reports to the same list.
    let mut session = session;
    session
        .reload_program(Some(config.to_string_lossy().as_ref()))
        .expect("reload the project");
    session.io_health().lock().unwrap().clear();
    scan(&session, 200);
    assert!(
        simulated(&session),
        "the reloaded runtime reports to the list the control server reads"
    );
    let _ = std::fs::remove_dir_all(root);
}

/// Specification 47 (`plant.status`): the simulator's simulation carries the revision of the project's
/// `simulation.toml`, as the runtime binary's does, so `plant.status` answers for its plants. Catches the simulator
/// answering "active revision unavailable", which left every plant card "Configured" while the plants ran.
#[test]
fn a_reloaded_simulator_gives_its_simulation_the_active_revision() {
    let simulation = "[simulation]\nenabled = true\ntime_scale = 1\n";
    let (root, config) = project("simulation-revision", &[("simulation.toml", simulation)]);
    let session = loaded_session(&root, &config);
    let expected = file_content_revision(root.join("simulation.toml")).expect("revision");
    let handle = session.simulation_handle();
    let guard = handle.lock().unwrap();
    let controller = guard
        .as_ref()
        .expect("an enabled simulation runs in the simulator");
    assert_eq!(
        controller.runtime_snapshot().active_configuration_revision,
        Some(expected)
    );
    drop(guard);
    let _ = std::fs::remove_dir_all(root);
}
