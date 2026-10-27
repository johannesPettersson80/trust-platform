use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use trust_runtime::control::ControlEndpoint;
use trust_runtime::io::IoDriverHealth;
use trust_runtime::scheduler::{ReloadClaim, ResourceCommand};

use super::*;

/// A project folder with one program, its configuration and the given project files; the second path is the
/// configuration's source, the program a launch names.
fn project(label: &str, files: &[(&str, &str)]) -> (PathBuf, PathBuf) {
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

/// The simulator of the project at `root` whose program is `config`, built and started as a launch starts it
/// (specification 44 NX-16).
fn start_simulator(
    root: &Path,
    config: &Path,
) -> (DebugSession, crate::adapter::test_simulator::Simulator) {
    crate::adapter::test_simulator::start(
        root,
        config,
        ControlEndpoint::Tcp("127.0.0.1:0".parse().expect("a loopback endpoint")),
    )
}

/// Waits until `healthy` holds for the driver health the simulator's control server reads.
fn wait_until_reported(
    simulator: &crate::adapter::test_simulator::Simulator,
    healthy: impl Fn(&[trust_runtime::io::IoDriverStatus]) -> bool,
) -> bool {
    let health = simulator.state().io_health.clone();
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if healthy(&health.lock().unwrap()) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    false
}

/// Specification 47 (Devices & Connections shows the simulator live): the simulator's control server reports its I/O
/// drivers' health in `fleet.topology`, as the runtime binary does. Catches the list the control server reads staying
/// empty, so the canvas drew the running simulator's I/O card as "Configured", and a reload (an online change of the
/// program, specification 44 §12, which keeps the launch's drivers) leaving the list behind.
#[test]
fn a_reloaded_simulator_reports_its_driver_health_to_the_control_list() {
    let (root, config) = project(
        "driver-health",
        &[("io.toml", "[io]\ndriver = \"simulated\"\n")],
    );
    let (mut session, simulator) = start_simulator(&root, &config);
    let simulated = |statuses: &[trust_runtime::io::IoDriverStatus]| {
        statuses
            .iter()
            .any(|status| status.name == "simulated" && status.health == IoDriverHealth::Ok)
    };
    let before_reload = wait_until_reported(&simulator, simulated);

    // A reload changes the program online; it reports to the same list.
    let sources = session
        .launch_sources(config.to_string_lossy().as_ref())
        .expect("the project's sources");
    let image =
        crate::adapter::test_simulator::build(&root, &sources, session.conversion_profile());
    let (sender, receiver) = std::sync::mpsc::channel();
    simulator
        .state()
        .resource
        .send_command(ResourceCommand::ReloadBytecode {
            bytes: image,
            claim: ReloadClaim::default(),
            respond_to: sender,
        })
        .expect("send the reload");
    let reloaded = receiver.recv_timeout(Duration::from_secs(10));
    simulator.state().io_health.lock().unwrap().clear();
    let after_reload = wait_until_reported(&simulator, simulated);
    simulator.stop();
    assert!(
        before_reload,
        "the scan reports the simulated driver as healthy"
    );
    assert!(
        matches!(reloaded, Ok(Ok(()))),
        "the program changes online: {reloaded:?}"
    );
    assert!(
        after_reload,
        "the reloaded program reports to the list the control server reads"
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
    let expected = file_content_revision(root.join("simulation.toml")).expect("revision");
    let (_session, simulator) = start_simulator(&root, &config);
    let (sender, receiver) = std::sync::mpsc::channel();
    simulator
        .state()
        .resource
        .send_command(ResourceCommand::SimulationSnapshot { respond_to: sender })
        .expect("send the snapshot request");
    let snapshot = receiver.recv_timeout(Duration::from_secs(2));
    simulator.stop();
    let controller = snapshot
        .expect("the scheduler answers")
        .expect("an enabled simulation runs in the simulator");
    assert_eq!(controller.active_configuration_revision, Some(expected));
    let _ = std::fs::remove_dir_all(root);
}
