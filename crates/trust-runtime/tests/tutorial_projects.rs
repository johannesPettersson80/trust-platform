//! The tutorials that build a project from an empty folder run as their READMEs write them
//! (checklist G21): the files a README writes with `cat > <file> <<'<TAG>'` are written as they
//! are, built and started with the real `trust-runtime`, and each step's expected result is read
//! back through `ctl`, as the README has the reader check it.

#[path = "support/runtime_process.rs"]
mod runtime_process;

use std::path::PathBuf;
use std::time::Duration;

use runtime_process::project_copy::copy_example;
use runtime_process::{describe, repo_root, trust_runtime_command, ExampleRuntime, Runtime};

/// How long a step waits for the runtime to show its expected result: a write is applied at a
/// cycle's start, and the tutorial's task runs every 100 ms.
const STEP: Duration = Duration::from_secs(10);

/// The files `readme` writes with `cat > <file> <<'<TAG>'`, each with its lines up to the line
/// `<TAG>`, in the README's order. A here-document in a numbered step is indented as the step's
/// text is: its lines lose the indentation of its `cat` line, as the reader's shell gets them.
fn here_documents(readme: &str) -> Vec<(String, String)> {
    let mut files = Vec::new();
    let mut lines = readme.lines();
    while let Some(line) = lines.next() {
        let command = line.trim_start();
        let indent = &line[..line.len() - command.len()];
        let Some((file, tag)) = command
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
            if body.trim_start() == tag {
                break;
            }
            text.push_str(body.strip_prefix(indent).unwrap_or(body));
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

#[test]
fn tutorial_19_drives_the_safe_state_at_a_fault_and_recovers_cold_as_its_readme_writes_it() {
    // examples/tutorials/19_safety_commissioning/README.md on tutorial 13's project: step 2 writes
    // src/Main.st, whose divisor `FaultTrigger` sets to 0, and src/Configuration.st, which puts
    // `FaultTrigger` at `%IX0.1`; step 3 writes io.toml (the loopback driver, the safe state
    // `%QX0.0` FALSE) and puts the watchdog and the fault policy `safe_halt` into runtime.toml;
    // step 5 sets `%IX0.0` and `%QX0.0` follows; step 6 sets `%IX0.1` and the division by zero
    // faults the resource; step 7 restarts it cold. Specification 11 §6.6: `safe_halt` drives the
    // safe values before the resource halts; its "safe state in the reads": the snapshot holds
    // the safe values, marked `"safe_state": true`, and the event `safe_state_applied` names
    // `%QX0.0` with `Bool(false)`; the next scan's snapshot carries no mark. Catches a tutorial
    // whose fault leaves the output as the last scan drove it, a fault that the events do not
    // name, and a cold restart that does not run the program again.
    let tutorial = repo_root().join("examples/tutorials");
    let readme19 = std::fs::read_to_string(tutorial.join("19_safety_commissioning/README.md"))
        .expect("tutorial 19's README");
    let mut files = here_documents(
        &std::fs::read_to_string(tutorial.join("13_project_bootstrap_zero_to_first_app/README.md"))
            .expect("tutorial 13's README"),
    );
    let replaced = here_documents(&readme19);
    assert_eq!(
        replaced
            .iter()
            .map(|(file, _)| file.as_str())
            .collect::<Vec<_>>(),
        ["src/Main.st", "src/Configuration.st", "io.toml"],
        "the files steps 2 and 3 write"
    );
    for (file, text) in replaced {
        let slot = files
            .iter_mut()
            .find(|(written, _)| *written == file)
            .expect("tutorial 13 writes the file that tutorial 19 replaces");
        slot.1 = text;
    }
    // The block stands in a step, indented as the step's text is.
    let policy: String = readme19
        .split("```toml\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("step 3's runtime.toml sections, the README's first TOML block")
        .lines()
        .map(|line| format!("{}\n", line.trim_start()))
        .collect();
    for (file, text) in &mut files {
        if file == "runtime.toml" {
            *text = with_sections(text, &policy);
        }
    }

    let runtime = ExampleRuntime::written("tutorial-19", files).start();
    set(&runtime, "%IX0.0", true);
    holds(
        &runtime,
        &[("%IX0.0", true), ("%QX0.0", true)],
        "step 5: %QX0.0 follows %IX0.0",
    );

    set(&runtime, "%IX0.1", true);
    runtime.wait_until(STEP, "step 6: the fault halts the resource", || {
        runtime.status().state == "faulted"
    });
    let status = runtime.status();
    assert_eq!(status.fault, "division by zero", "step 6");
    let snapshot = runtime.ctl_result(&["io-read"])["snapshot"].clone();
    assert_eq!(
        snapshot["safe_state"], true,
        "step 6: the safe state's snapshot\n{snapshot}"
    );
    let output = snapshot["outputs"]
        .as_array()
        .and_then(|outputs| outputs.iter().find(|entry| entry["address"] == "%QX0.0"))
        .unwrap_or_else(|| panic!("step 6: no %QX0.0 in\n{snapshot}"));
    assert_eq!(
        output["value"], "Bool(false)",
        "step 6: the safe value of %QX0.0"
    );
    let events = runtime.ctl_result(&["events"])["events"].clone();
    let listed = events.as_array().cloned().unwrap_or_default();
    assert!(
        listed
            .iter()
            .any(|event| event["type"] == "fault" && event["error"] == "division by zero"),
        "step 6: the events name the fault\n{events}"
    );
    assert!(
        listed.iter().any(|event| {
            event["type"] == "safe_state_applied"
                && event["outputs"]
                    == serde_json::json!([{"address": "%QX0.0", "value": "Bool(false)"}])
        }),
        "step 6: the events name the safe state of %QX0.0\n{events}"
    );

    runtime.ctl_ok(&["restart", "cold"]);
    runtime.wait_until(
        STEP,
        "step 7: the cold restart runs the program again",
        || runtime.status().state == "running",
    );
    set(&runtime, "%IX0.0", true);
    holds(
        &runtime,
        &[("%IX0.0", true), ("%IX0.1", false), ("%QX0.0", true)],
        "step 7: after the cold restart %QX0.0 follows %IX0.0 again",
    );
    assert!(
        runtime.ctl_result(&["io-read"])["snapshot"]
            .get("safe_state")
            .is_none(),
        "step 7: a scan's snapshot carries no safe-state mark"
    );
}

/// The `trust-runtime build` and `trust-runtime validate` lines of the README at `readme` (a path
/// under the repository), each split into its arguments.
fn gate_commands(readme: &str) -> Vec<Vec<String>> {
    std::fs::read_to_string(repo_root().join(readme))
        .unwrap_or_else(|error| panic!("{readme}: {error}"))
        .lines()
        .filter_map(|line| line.strip_prefix("trust-runtime "))
        .filter(|args| args.starts_with("build ") || args.starts_with("validate "))
        .map(|args| args.split_whitespace().map(str::to_string).collect())
        .collect()
}

/// Runs `commands` in order as a reader runs them from the repository root: each from a root holding
/// a copy of the example its `--project` names, made once, so a `validate` finds what the `build`
/// before it wrote. Each must succeed (exit code 0).
fn assert_each_succeeds(commands: &[Vec<String>]) {
    let mut copies: Vec<(String, PathBuf)> = Vec::new();
    let mut failures = Vec::new();
    for args in commands {
        let project = args
            .iter()
            .skip_while(|arg| *arg != "--project")
            .nth(1)
            .unwrap_or_else(|| panic!("{args:?} names its project"));
        let example = project
            .strip_prefix("examples/")
            .unwrap_or_else(|| panic!("{project} is an example under examples/"));
        let root = match copies.iter().find(|(copied, _)| copied == project) {
            Some((_, root)) => root.clone(),
            None => {
                let root = copy_example(example);
                copies.push((project.clone(), root.clone()));
                root
            }
        };
        let output = trust_runtime_command()
            .args(args)
            .current_dir(&root)
            .output()
            .expect("run trust-runtime");
        if !output.status.success() {
            failures.push(format!(
                "`trust-runtime {}`: {}",
                args.join(" "),
                describe(&output)
            ));
        }
    }
    for (_, root) in &copies {
        let _ = std::fs::remove_dir_all(root);
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn tutorial_21_gates_pass_on_the_project_it_names() {
    // examples/tutorials/21_ci_cd_project_pipeline/README.md: from the repository root, step 2 runs
    // `trust-runtime build --project examples/tutorials/11_unit_testing_102 --ci` and step 3
    // `trust-runtime validate --project examples/tutorials/11_unit_testing_102 --ci`, and each is
    // expected to succeed (exit code 0 in step 6's table). Catches a project the gates refuse:
    // tutorial 11 had no runtime.toml or io.toml, and step 3 exited 10 ("missing runtime.toml").
    let commands = gate_commands("examples/tutorials/21_ci_cd_project_pipeline/README.md");
    assert_eq!(
        commands,
        [
            [
                "build",
                "--project",
                "examples/tutorials/11_unit_testing_102",
                "--ci"
            ],
            [
                "validate",
                "--project",
                "examples/tutorials/11_unit_testing_102",
                "--ci"
            ],
        ],
        "the gates of steps 2 and 3"
    );
    assert_each_succeeds(&commands);
}

#[test]
fn tutorial_22_terminal_build_and_validate_succeed() {
    // examples/tutorials/22_neovim_zed_workflow/README.md, step 5: from the repository root, `trust-
    // runtime build` and `trust-runtime validate` of the project the step names give "the same build
    // confidence as VS Code users". Catches commands that cannot succeed: the step named tutorial 10,
    // a test-only project without a configuration or runtime.toml, and `--sources
    // examples/tutorials/10_unit_testing_101/sources`, which `build` resolves inside the project
    // ("sources directory not found").
    let commands = gate_commands("examples/tutorials/22_neovim_zed_workflow/README.md");
    assert_eq!(
        commands
            .iter()
            .map(|args| args[0].as_str())
            .collect::<Vec<_>>(),
        ["build", "validate"],
        "step 5's build and validate"
    );
    assert_each_succeeds(&commands);
}

/// `runtime` with each section of the TOML `block` put in: a section `runtime` has is replaced, from
/// its header to the next one, and another is added at the end, as a reader edits the file.
fn with_sections(runtime: &str, block: &str) -> String {
    let mut text = runtime.to_string();
    let mut sections: Vec<(String, String)> = Vec::new();
    for line in block.lines() {
        if line.starts_with('[') {
            sections.push((line.to_string(), String::new()));
        }
        if let Some((_, body)) = sections.last_mut() {
            body.push_str(line);
            body.push('\n');
        }
    }
    for (header, body) in sections {
        let lines: Vec<&str> = text.lines().collect();
        match lines.iter().position(|line| *line == header) {
            Some(start) => {
                let end = lines[start + 1..]
                    .iter()
                    .position(|line| line.starts_with('['))
                    .map_or(lines.len(), |offset| start + 1 + offset);
                let mut edited: Vec<String> =
                    lines[..start].iter().map(|l| l.to_string()).collect();
                edited.extend(body.lines().map(str::to_string));
                edited.push(String::new());
                edited.extend(lines[end..].iter().map(|l| l.to_string()));
                text = edited.join("\n") + "\n";
            }
            None => {
                text.push('\n');
                text.push_str(&body);
            }
        }
    }
    text
}

#[test]
fn tutorial_16_configuration_builds_and_validates() {
    // examples/tutorials/16_secure_remote_access/README.md: on tutorial 13's project, a reader puts
    // the README's configuration, its first TOML block (step 4), into runtime.toml, with the token
    // of step 2 in place of the placeholder and the certificate of step 3 in security/ (here the
    // tests' fixture certificate), then builds and validates (step 6: `Project ok`). Specification 11: a `tcp://` control
    // endpoint admits only a loopback address, and `runtime.web.auth = "token"` requires
    // `runtime.control.auth_token`. Catches a tutorial whose configuration the runtime refuses, as
    // its remote control endpoint `tcp://0.0.0.0:17777` was ("tcp endpoint must be loopback").
    let readme = std::fs::read_to_string(
        repo_root().join("examples/tutorials/16_secure_remote_access/README.md"),
    )
    .expect("tutorial 16's README");
    let token = "0123456789abcdef".repeat(3);
    let files = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
        )
        .expect("tutorial 13's README"),
    );
    let project = trust_test_support::unique_temp_path("tutorial-16");
    for (file, text) in &files {
        let path = project.join(file);
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(&path, text).expect("the file");
    }
    let runtime =
        std::fs::read_to_string(project.join("runtime.toml")).expect("tutorial 13's runtime.toml");
    let configuration = readme
        .split("```toml\n")
        .nth(1)
        .and_then(|block| block.split("```").next())
        .expect("the README's configuration, its first TOML block");
    let runtime = with_sections(&runtime, configuration);
    let runtime = runtime
        .replace("paste-the-token-from-step-2-here", &token)
        .replace("replace-with-long-random-token", &token);
    std::fs::write(project.join("runtime.toml"), &runtime).expect("runtime.toml");
    let fixtures = repo_root().join("crates/trust-runtime/tests/fixtures/tls");
    std::fs::create_dir_all(project.join("security")).expect("security/");
    for file in ["server-cert.pem", "server-key.pem"] {
        std::fs::copy(fixtures.join(file), project.join("security").join(file))
            .expect("the certificate");
    }
    let outputs: Vec<_> = [
        vec!["build", "--project", ".", "--sources", "src"],
        vec!["validate", "--project", "."],
    ]
    .iter()
    .map(|args| {
        (
            args.join(" "),
            trust_runtime_command()
                .args(args)
                .current_dir(&project)
                .output()
                .expect("run trust-runtime"),
        )
    })
    .collect();
    let _ = std::fs::remove_dir_all(&project);
    for (args, output) in outputs {
        assert!(
            output.status.success(),
            "`trust-runtime {args}` with\n{runtime}\n{}",
            describe(&output)
        );
    }
}

#[test]
fn tutorial_15_configurations_build_and_validate() {
    // examples/tutorials/15_multi_plc_discovery_mesh/README.md: step 1 copies tutorial 13's project
    // to /tmp/trust-plc-a and /tmp/trust-plc-b, steps 2 and 3 write PLC-A's src/Main.st and both
    // runtime.toml files with here-documents, and step 4 builds and validates both (`Project ok`).
    // Catches a configuration the runtime refuses, as the tutorial's `auth_token = ""` in both meshes
    // was ("runtime.mesh.auth_token must not be empty").
    let tutorial_13 = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/13_project_bootstrap_zero_to_first_app/README.md"),
        )
        .expect("tutorial 13's README"),
    );
    let tutorial_15 = here_documents(
        &std::fs::read_to_string(
            repo_root().join("examples/tutorials/15_multi_plc_discovery_mesh/README.md"),
        )
        .expect("tutorial 15's README"),
    );
    let mut results = Vec::new();
    for plc in ["a", "b"] {
        let copy = format!("/tmp/trust-plc-{plc}/");
        assert!(
            tutorial_15
                .iter()
                .any(|(file, _)| *file == format!("{copy}runtime.toml")),
            "step 3 writes PLC-{plc}'s runtime.toml"
        );
        let project = trust_test_support::unique_temp_path(&format!("tutorial-15-{plc}"));
        let written = tutorial_13
            .iter()
            .cloned()
            .chain(tutorial_15.iter().filter_map(|(file, text)| {
                file.strip_prefix(&copy)
                    .map(|relative| (relative.to_string(), text.clone()))
            }));
        for (file, text) in written {
            let path = project.join(&file);
            std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
            std::fs::write(&path, text).expect("the file");
        }
        for args in [
            vec!["build", "--project", ".", "--sources", "src"],
            vec!["validate", "--project", "."],
        ] {
            let output = trust_runtime_command()
                .args(&args)
                .current_dir(&project)
                .output()
                .expect("run trust-runtime");
            results.push((
                format!("PLC-{plc}: trust-runtime {}", args.join(" ")),
                output,
            ));
        }
        let _ = std::fs::remove_dir_all(&project);
    }
    for (what, output) in results {
        assert!(output.status.success(), "{what}: {}", describe(&output));
    }
}

#[path = "tutorial_projects/io_backends.rs"]
mod io_backends;

#[cfg(unix)]
#[path = "tutorial_projects/deploy_and_rollback.rs"]
mod deploy_and_rollback;

#[path = "tutorial_projects/observability.rs"]
mod observability;

#[path = "tutorial_projects/mesh_sharing.rs"]
mod mesh_sharing;
