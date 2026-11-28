//! Tutorial 18 (simulation with `simulation.toml`) as its README writes it: its project's schedule
//! on the simulation clock, which `trust-runtime simulate` moves, and its Try-its.

use super::simulator_support::Simulator;
use super::*;
use serde_json::{json, Value};

/// The README of tutorial `folder`.
fn tutorial_readme(folder: &str) -> String {
    std::fs::read_to_string(repo_root().join(format!("examples/tutorials/{folder}/README.md")))
        .expect("a tutorial's README")
}

/// Tutorial 13's files with tutorial 18's in their place, and tutorial 18's `simulation.toml`.
fn tutorial_18_files() -> Vec<(String, String)> {
    let mut files = here_documents(&tutorial_readme("13_project_bootstrap_zero_to_first_app"));
    for (file, text) in here_documents(&tutorial_readme("18_simulation_toml_fault_injection")) {
        match files.iter_mut().find(|(existing, _)| *existing == file) {
            Some(entry) => entry.1 = text,
            None => files.push((file, text)),
        }
    }
    files
}

/// Writes `files` into a new project folder and builds it as step 2 does.
fn built_project(files: &[(String, String)]) -> PathBuf {
    let project = trust_test_support::unique_temp_path("tutorial-18");
    for (file, text) in files {
        let path = project.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(&path, text).expect("the file");
    }
    let built = trust_runtime_command()
        .args(["build", "--project", ".", "--sources", "src"])
        .current_dir(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        built.status.success(),
        "step 2's build: {}",
        describe(&built)
    );
    project
}

/// Advances the simulator's clock by `ms` milliseconds and gives the advance's answer.
fn advance(simulator: &mut Simulator, id: u64, ms: u64) -> Value {
    simulator.request(
        id,
        "simulator.advance",
        json!({ "duration_ns": ms * 1_000_000 }),
    )
}

/// `StartCmd`, `LampFeedback` and `LampOut`, as the last completed scan left them.
fn lamp(simulator: &mut Simulator, id: u64) -> Value {
    let read = simulator.request(
        id,
        "variables.read",
        json!({ "paths": ["P1.StartCmd", "P1.LampFeedback", "P1.LampOut"] }),
    );
    Value::Array(
        read["values"]
            .as_array()
            .expect("the values")
            .iter()
            .map(|value| value["value"].clone())
            .collect(),
    )
}

#[test]
fn tutorial_18_runs_its_schedule_on_the_simulation_clock() {
    // examples/tutorials/18_simulation_toml_fault_injection/README.md, steps 3 to 6 (specification
    // 11 §6.9.4: the disturbances and coupling effects that are due are written before a cycle,
    // the couplings queue a changed output for their delay after it, and a fault disturbance faults
    // the runtime at its due time; §6.8.1, the simulator's clock, which only the test moves). The
    // start button is pressed at 5 s and released at 15 s, and the coupling feeds LampOut back to
    // LampFeedback 3 s later: [StartCmd, LampFeedback, LampOut] read [TRUE, FALSE, TRUE] at 6.5 s,
    // all TRUE at 9 s, [FALSE, TRUE, FALSE] at 16.5 s and all FALSE at 19.5 s. The advance towards
    // 26.5 s ends at the cycle of 25 s with the fault `lamp circuit open`. Catches a coupling that
    // ignores its delay, a disturbance at the wrong time, and a fault disturbance that does nothing.
    let project = built_project(&tutorial_18_files());
    let mut simulator = Simulator::start(&project);
    advance(&mut simulator, 1, 6_500);
    assert_eq!(lamp(&mut simulator, 2), json!([true, false, true]), "6.5 s");
    advance(&mut simulator, 3, 2_500);
    assert_eq!(lamp(&mut simulator, 4), json!([true, true, true]), "9 s");
    advance(&mut simulator, 5, 7_500);
    assert_eq!(
        lamp(&mut simulator, 6),
        json!([false, true, false]),
        "16.5 s"
    );
    advance(&mut simulator, 7, 3_000);
    assert_eq!(
        lamp(&mut simulator, 8),
        json!([false, false, false]),
        "19.5 s"
    );
    let faulted = advance(&mut simulator, 9, 7_000);
    drop(simulator);
    let _ = std::fs::remove_dir_all(&project);
    assert_eq!(faulted["state"], json!("faulted"), "{faulted}");
    assert_eq!(faulted["time_ns"], json!(25_000_000_000u64), "{faulted}");
    assert_eq!(
        faulted["fault"]["error"],
        json!("simulation fault 'lamp circuit open'"),
        "{faulted}"
    );
}

#[test]
fn tutorial_18_try_its_report_the_mode_and_refuse_an_unknown_kind() {
    // The README's Try-its (specification 11, the launch's simulation selection: the file's
    // `enabled` is authoritative unless `--simulation` turns it on; §6.9.4.1: unknown kinds are
    // rejected): with `enabled = false` the runtime reports `simulation.enabled` false, with
    // `--simulation` it reports the simulation mode and its warning; `kind = "dropout"` is refused
    // by `validate` with the README's error. Catches a file whose `enabled = false` is ignored, a
    // `--simulation` that does not turn the model on, and an unknown kind accepted.
    let readme = tutorial_readme("18_simulation_toml_fault_injection");
    let files = tutorial_18_files();
    let disabled: Vec<(String, String)> = files
        .iter()
        .map(|(file, text)| {
            let text = if file == "simulation.toml" {
                text.replace("enabled = true", "enabled = false")
            } else {
                text.clone()
            };
            (file.clone(), text)
        })
        .collect();
    let production = ExampleRuntime::written("tutorial-18-disabled", disabled.clone()).start();
    let config = production.ctl_result(&["config-get"]);
    assert_eq!(config["simulation.enabled"], json!(false), "{config}");
    drop(production);
    let simulated = ExampleRuntime::written("tutorial-18-simulation", disabled)
        .play_arg("--simulation")
        .start();
    let config = simulated.ctl_result(&["config-get"]);
    assert_eq!(config["simulation.mode"], json!("simulation"), "{config}");
    assert_eq!(
        config["simulation.warning"],
        json!("Simulation mode active (time scale x1). Not for live hardware."),
        "{config}"
    );
    assert!(
        readme.contains("\"simulation.enabled\":false")
            && readme.contains("Simulation mode active (time scale x1). Not for live hardware."),
        "the README quotes both reports"
    );
    drop(simulated);

    let expected = "Error: invalid config 'unsupported disturbance kind 'dropout''";
    let dropout: Vec<(String, String)> = files
        .into_iter()
        .map(|(file, text)| {
            let text = if file == "simulation.toml" {
                text.replace("kind = \"fault\"", "kind = \"dropout\"")
            } else {
                text
            };
            (file, text)
        })
        .collect();
    let project = built_project(&dropout);
    let refused = trust_runtime_command()
        .args(["validate", "--project", "."])
        .current_dir(&project)
        .output()
        .expect("run trust-runtime validate");
    let _ = std::fs::remove_dir_all(&project);
    assert!(readme.contains(expected), "the README quotes `{expected}`");
    assert!(!refused.status.success(), "the unknown kind is refused");
    assert!(
        String::from_utf8_lossy(&refused.stderr).contains(expected),
        "validate's error: {}",
        describe(&refused)
    );
}
