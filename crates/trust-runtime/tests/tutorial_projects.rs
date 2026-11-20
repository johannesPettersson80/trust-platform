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

/// Sets `%IX0.0` as the README has the reader set it (`ctl io-write`).
fn set_start(runtime: &Runtime, value: bool) {
    runtime.ctl_ok(&["io-write", "%IX0.0", if value { "true" } else { "false" }]);
}

/// `%IX0.0` and `%QX0.0` of the runtime's latest I/O snapshot (`ctl io-read`), once there is one.
fn start_and_lamp(runtime: &Runtime) -> Option<(String, String)> {
    let snapshot = runtime.snapshot()?;
    Some((snapshot.text("%IX0.0"), snapshot.text("%QX0.0")))
}

/// The printed value of a BOOL in an I/O snapshot.
fn printed(value: bool) -> String {
    format!("Bool({value})")
}

/// Waits until `%IX0.0` and `%QX0.0` read `start` and `lamp`, then requires them at every snapshot of
/// the next five cycles: a driver that writes the inputs itself makes them change from one cycle to
/// the next, which a single read could miss.
fn holds(runtime: &Runtime, start: bool, lamp: bool, what: &str) {
    let expected = Some((printed(start), printed(lamp)));
    runtime.wait_until(STEP, what, || start_and_lamp(runtime) == expected);
    let first = runtime.scan();
    loop {
        let snapshot = runtime.snapshot().expect("a snapshot");
        assert_eq!(
            Some((snapshot.text("%IX0.0"), snapshot.text("%QX0.0"))),
            expected,
            "{what}, at scan {} of the five after {first}",
            snapshot.scan()
        );
        if snapshot.scan() >= first + 5 {
            break;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
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
        set_start(&runtime, value);
        holds(
            &runtime,
            value,
            value,
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
        set_start(&inverted, value);
        holds(
            &inverted,
            value,
            !value,
            &format!("step 9: %QX0.0 is NOT %IX0.0 = {value}"),
        );
    }
}
