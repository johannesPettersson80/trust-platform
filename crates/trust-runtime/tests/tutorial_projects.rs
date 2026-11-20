//! The tutorials that build a project from an empty folder run as their READMEs write them
//! (checklist G21): the files a README writes with `cat > <file> <<'<TAG>'` are written as they
//! are, built and started with the real `trust-runtime`, and each step's expected result is read
//! back through `ctl`, as the README has the reader check it.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::time::Duration;

use runtime_process::{repo_root, ExampleRuntime, Runtime};

/// How long a step waits for the runtime to show its expected result: a write is applied at a
/// cycle's start, and the tutorial's task runs every 100 ms.
const STEP: Duration = Duration::from_secs(10);

/// The files `readme` writes with `cat > <file> <<'<TAG>'`, each with its lines up to the line
/// `<TAG>`, in the README's order.
fn here_documents(readme: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut lines = readme.lines();
    while let Some(line) = lines.next() {
        let Some((file, tag)) = line
            .strip_prefix("cat > ")
            .and_then(|rest| rest.split_once(" <<'"))
        else {
            continue;
        };
        let Some(tag) = tag.strip_suffix('\'') else {
            continue;
        };
        let mut text = String::new();
        for body in lines.by_ref() {
            if body == tag {
                break;
            }
            text.push_str(body);
            text.push('\n');
        }
        files.push((file.to_string(), text));
    }
    files
}

/// Sets the input `address` as the README has the reader set it (`ctl io-write`).
fn set(runtime: &Runtime, address: &str, value: bool) {
    runtime.ctl_ok(&["io-write", address, if value { "true" } else { "false" }]);
}

/// The printed value of a BOOL in an I/O snapshot.
fn printed(value: bool) -> String {
    format!("Bool({value})")
}

/// The printed values of `addresses` in the runtime's latest I/O snapshot (`ctl io-read`), once
/// there is one.
fn read(runtime: &Runtime, addresses: &[&str]) -> Option<Vec<String>> {
    let snapshot = runtime.snapshot()?;
    Some(
        addresses
            .iter()
            .map(|address| snapshot.text(address))
            .collect(),
    )
}

/// Waits until each address of `expected` reads its value, then requires them at every snapshot of
/// the next five cycles: a driver that writes the inputs itself makes them change from one cycle to
/// the next, which a single read could miss.
fn holds(runtime: &Runtime, expected: &[(&str, bool)], what: &str) {
    let addresses: Vec<&str> = expected.iter().map(|(address, _)| *address).collect();
    let values: Vec<String> = expected.iter().map(|(_, value)| printed(*value)).collect();
    runtime.wait_until(STEP, what, || {
        read(runtime, &addresses).as_ref() == Some(&values)
    });
    let first = runtime.scan();
    loop {
        let snapshot = runtime.snapshot().expect("a snapshot");
        let now: Vec<String> = addresses
            .iter()
            .map(|address| snapshot.text(address))
            .collect();
        assert_eq!(
            now,
            values,
            "{what}, at scan {} of the five after {first}",
            snapshot.scan()
        );
        if snapshot.scan() >= first + 5 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
}

/// The value of the program variable `path` as `ctl read` prints it: the text after ` = `.
fn variable(runtime: &Runtime, path: &str) -> String {
    let printed = runtime.ctl_ok(&["read", path]);
    printed
        .lines()
        .find_map(|line| {
            line.strip_prefix(&format!("{path} : "))
                .and_then(|rest| rest.split_once(" = "))
                .map(|(_, value)| value.to_string())
        })
        .unwrap_or_else(|| panic!("`ctl read {path}` printed no value: {printed}"))
}

#[test]
fn tutorial_13_runs_as_its_readme_writes_it() {
    // examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md, worked from its steps:
    // steps 2 to 5 write src/Main.st, src/Configuration.st, runtime.toml and io.toml; step 6
    // builds; in step 8 `%QX0.0` follows `%IX0.0` set TRUE, then FALSE; step 9 writes
    // `LampOut := NOT StartCmd;`, builds and runs again, and the output is the input inverted, each
    // input staying as it was set. Catches a README whose files the runtime refuses (an empty
    // `runtime.mesh.auth_token` failed step 6) and a driver that writes the inputs itself
    // (`loopback` copies `%QX0.0` into `%IX0.0` at every cycle, which fights step 9).
    let readme = std::fs::read_to_string(
        repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
    )
    .expect("the README");
    let files = here_documents(&readme);
    assert_eq!(
        files
            .iter()
            .map(|(file, _)| file.as_str())
            .collect::<Vec<_>>(),
        [
            "src/Main.st",
            "src/Configuration.st",
            "runtime.toml",
            "io.toml"
        ],
        "the files steps 2 to 5 write"
    );

    let runtime = ExampleRuntime::written("tutorial-13", files.clone()).start();
    for value in [true, false] {
        set(&runtime, "%IX0.0", value);
        holds(
            &runtime,
            &[("%IX0.0", value), ("%QX0.0", value)],
            &format!("step 8: %QX0.0 follows %IX0.0 = {value}"),
        );
    }
    drop(runtime);

    let inverted = ExampleRuntime::written("tutorial-13", files)
        .edit_source(
            "src/Main.st",
            "LampOut := StartCmd;",
            "LampOut := NOT StartCmd;",
        )
        .start();
    for value in [true, false] {
        set(&inverted, "%IX0.0", value);
        holds(
            &inverted,
            &[("%IX0.0", value), ("%QX0.0", !value)],
            &format!("step 9: %QX0.0 is NOT %IX0.0 = {value}"),
        );
    }
}

#[test]
fn tutorial_12_scenarios_run_as_its_readme_describes_them() {
    // examples/tutorials/12_hmi_pid_process_dashboard/README.md, its scenario checks in order, read
    // through the outputs its step 4 maps (%QX0.0 PumpRunning, %QX0.1 HighPressureAlarm, %QX0.4
    // BypassOpen) and the texts src/main.st writes: (1) %IX0.0 TRUE starts the transfer: the pump
    // runs, ModeText 'RUNNING: NORMAL MODE'; (2) %IX0.3 TRUE opens the bypass, ModeText 'RUNNING:
    // BYPASS MODE'; (3) a pulse of %IX0.2 raises HighPressureAlarm, AlarmMessage 'ALARM: PT-001 HIGH
    // PRESSURE', which clears when the pulse ends; (4) %IX0.1 TRUE stops: the pump and the bypass are
    // off, ModeText 'STOPPED'. Each input holds as set. Catches the `loopback` driver the project
    // had, which copies the outputs over these inputs at every cycle: the bypass command never held,
    // and BypassOpen never rose.
    let runtime = ExampleRuntime::new("tutorials/12_hmi_pid_process_dashboard").start();
    set(&runtime, "%IX0.0", true);
    holds(
        &runtime,
        &[("%IX0.0", true), ("%QX0.0", true)],
        "scenario 1: the pump runs",
    );
    assert_eq!(
        variable(&runtime, "P1.ModeText"),
        "\"RUNNING: NORMAL MODE\""
    );
    set(&runtime, "%IX0.3", true);
    holds(
        &runtime,
        &[("%IX0.3", true), ("%QX0.4", true)],
        "scenario 2: the bypass opens",
    );
    assert_eq!(
        variable(&runtime, "P1.ModeText"),
        "\"RUNNING: BYPASS MODE\""
    );
    set(&runtime, "%IX0.2", true);
    holds(
        &runtime,
        &[("%IX0.2", true), ("%QX0.1", true)],
        "scenario 3: the pressure spike raises the alarm",
    );
    assert_eq!(
        variable(&runtime, "P1.AlarmMessage"),
        "\"ALARM: PT-001 HIGH PRESSURE\""
    );
    set(&runtime, "%IX0.2", false);
    holds(
        &runtime,
        &[("%IX0.2", false), ("%QX0.1", false)],
        "scenario 3: the alarm clears when the pulse ends",
    );
    set(&runtime, "%IX0.1", true);
    holds(
        &runtime,
        &[("%IX0.1", true), ("%QX0.0", false), ("%QX0.4", false)],
        "scenario 4: the transfer stops",
    );
    assert_eq!(variable(&runtime, "P1.ModeText"), "\"STOPPED\"");
}

#[test]
fn tutorial_18_runs_its_simulation_as_its_readme_writes_it() {
    // examples/tutorials/18_simulation_toml_fault_injection/README.md: step 1 copies tutorial 13's
    // project, step 3 adds its simulation.toml (a coupling of %QX0.0 back to %IX0.0, two timed
    // inputs and a fault at 1,800 ms of simulated time), and step 4 runs `play --simulation
    // --time-scale 8`; step 5 has the reader confirm the injected fault's message. Catches a
    // simulation.toml the runtime refuses, as the step's coupling with `on_true` and `on_false`
    // and no threshold was ("coupling on_true/on_false require threshold").
    let mut files = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
        )
        .expect("tutorial 13's README"),
    );
    files.extend(here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/18_simulation_toml_fault_injection/README.md"),
        )
        .expect("tutorial 18's README"),
    ));
    assert!(
        files.iter().any(|(file, _)| file == "simulation.toml"),
        "step 3 writes simulation.toml"
    );
    let runtime = ExampleRuntime::written("tutorial-18", files)
        .play_arg("--simulation")
        .play_arg("--time-scale")
        .play_arg("8")
        .start();
    runtime.wait_until(STEP, "step 5: the injected fault is reported", || {
        runtime.ctl_result(&["faults"])["faults"]
            .as_array()
            .is_some_and(|faults| {
                faults.iter().any(|fault| {
                    fault["error"]
                        .as_str()
                        .is_some_and(|error| error.contains("tutorial simulated input dropout"))
                })
            })
    });
}
