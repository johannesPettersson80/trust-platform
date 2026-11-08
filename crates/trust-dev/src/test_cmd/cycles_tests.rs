//! Multi-cycle tests (specification 22, "Multi-cycle tests"; specification 07, `TEST_FINISHED`), run compiled for this computer:
//! each expected cycle and time is worked by hand from the specification's rule that cycle `n` runs at the time `(n − 1)·cycle`.

use super::*;

/// The results of running the tests `run` of a project whose only source is `source`.
fn results(label: &str, source: &str, run: &[&str]) -> Vec<ExecutedTest> {
    results_with_simulation(label, source, None, run)
}

/// The results of running the tests `run` of a project whose only source is `source`, with `simulation` as its
/// `simulation.toml` when given.
fn results_with_simulation(
    label: &str,
    source: &str,
    simulation: Option<&str>,
    run: &[&str],
) -> Vec<ExecutedTest> {
    let project = trust_test_support::unique_temp_path(&format!(
        "trust-dev-cycles-{label}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(project.join("src/tests.st"), source).expect("write the tests");
    if let Some(simulation) = simulation {
        std::fs::write(project.join("simulation.toml"), simulation).expect("write the simulation");
    }
    let sources_root = resolve_sources_root(&project, None).expect("sources");
    let sources = load_sources(&sources_root).expect("load");
    let discovered = discover_tests(&sources);
    let tests: Vec<DiscoveredTest> = run
        .iter()
        .map(|name| {
            discovered
                .iter()
                .find(|case| case.name.as_str() == *name)
                .unwrap_or_else(|| panic!("{name} is discovered"))
                .clone()
        })
        .collect();
    let results = execute_native_tests(&project, &discovered, &tests, 5).expect("the compiled run");
    let _ = std::fs::remove_dir_all(&project);
    results
}

/// The outcome, message and cycles of `result`.
fn outcome(result: &ExecutedTest) -> (TestOutcome, String, Option<(u32, u64)>) {
    (
        result.outcome,
        result.message.clone().unwrap_or_default(),
        result
            .cycles
            .map(|run| (run.cycles, run.time_ns / 1_000_000)),
    )
}

/// A timer test: `TON` with `PT := T#50ms`, started in the first cycle. At the default 10 ms cycle, cycle `n` runs at
/// `(n − 1)·10 ms`, so the timer's elapsed time reaches 50 ms in cycle 6, which finishes the test there at 50 ms. Catches a test
/// that runs its body once (the timer never ends), a time taken from the wall clock (the six cycles take microseconds), and a
/// test that runs on after `TEST_FINISHED()`.
#[test]
fn a_multi_cycle_test_runs_in_simulated_time_until_it_calls_test_finished() {
    let source = r#"
TEST_PROGRAM TimerEnds
VAR
    Delay : TON;
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
Delay(IN := TRUE, PT := T#50ms);
IF Delay.Q THEN
    ASSERT_EQUAL(EXPECTED := INT#6, ACTUAL := Cycles);
    TEST_FINISHED();
END_IF
END_TEST_PROGRAM
"#;
    let results = results("timer", source, &["TimerEnds"]);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((6, 50)))
    );
}

/// The budget (`test_max_cycles`) bounds a test that never finishes: 20 cycles, the last at `19·10 ms = 190 ms`, and the test
/// fails with that. Catches a test that runs on past its budget (a hang), and one that passes because nothing failed.
#[test]
fn a_test_that_does_not_finish_within_its_budget_fails() {
    let source = r#"
{attribute 'test_max_cycles' := '20'}
TEST_PROGRAM NeverFinishes
VAR
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
END_TEST_PROGRAM
"#;
    let results = results("budget", source, &["NeverFinishes"]);
    assert_eq!(
        outcome(&results[0]),
        (
            TestOutcome::Failed,
            "did not finish within 20 cycles (t = T#190ms)".to_owned(),
            Some((20, 190))
        )
    );
}

/// A failed assertion ends a multi-cycle test at its cycle, named with the time: at a 100 ms cycle (`test_cycle`), the count
/// reaches 3 in cycle 3, at `2·100 ms = 200 ms`. A `PROGRAM` with `{attribute 'test'}`, the CODESYS form, runs alike. Catches the
/// cycle or the time missing from the message, the cycle attribute ignored (the time would read 20 ms), and the attribute form
/// running once.
#[test]
fn a_failed_assertion_names_its_cycle_and_time() {
    let source = r#"
{attribute 'test'}
{attribute 'test_cycle' := 'T#100ms'}
PROGRAM PRG_TestCountStaysBelowThree
VAR
    Count : INT;
END_VAR
Count := Count + 1;
ASSERT_TRUE(Count < 3);
IF Count >= 10 THEN
    TEST_FINISHED();
END_IF
END_PROGRAM
"#;
    let results = results("assertion", source, &["PRG_TestCountStaysBelowThree"]);
    assert_eq!(
        outcome(&results[0]),
        (
            TestOutcome::Failed,
            "ASSERT_TRUE expected TRUE, got FALSE (cycle 3, t = T#200ms)".to_owned(),
            Some((3, 200))
        )
    );
}

/// A test that neither calls `TEST_FINISHED()` nor names a cycle runs its body once, as before, and reports no cycles. Catches
/// every test turned into a multi-cycle one (each would fail its budget).
#[test]
fn a_test_without_test_finished_runs_once() {
    let source = r#"
TEST_PROGRAM RunsOnce
VAR
    Cycles : INT;
END_VAR
Cycles := Cycles + 1;
ASSERT_EQUAL(EXPECTED := INT#1, ACTUAL := Cycles);
END_TEST_PROGRAM
"#;
    let results = results("once", source, &["RunsOnce"]);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), None)
    );
}

/// A device behind a TCP bridge that steps as specification 11 §6.8.1 says and runs nothing: its resource state follows
/// `restart` and `shutdown`, an install's transfer is taken whole, a cold restart after an install starts the stepped time
/// at zero, and a step whose time is before the previous step's is refused (`time before the last step`). The finished mark
/// reads TRUE from the sixth step after a cold restart, as in a test that finishes in its sixth cycle. Records each step's
/// time.
fn stepping_device() -> (String, std::sync::Arc<std::sync::Mutex<Vec<u64>>>) {
    use std::io::{BufRead, Write as _};
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind the device's bridge");
    let port = format!(
        "tcp://{}",
        listener.local_addr().expect("the bridge's address")
    );
    let steps = std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let recorded = std::sync::Arc::clone(&steps);
    std::thread::spawn(move || {
        let (stream, _) = listener.accept().expect("the link connects");
        let mut replies = stream.try_clone().expect("the bridge's stream");
        let (mut state, mut last, mut installed, mut since_cold) =
            ("stopped", None::<u64>, false, 0_u32);
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let request: serde_json::Value = serde_json::from_str(&line).expect("a JSON request");
            let params = &request["params"];
            let result = match request["type"].as_str().unwrap_or_default() {
                "status" => Ok(json!({"state": state, "version": env!("CARGO_PKG_VERSION")})),
                "restart" => {
                    state = "running";
                    Ok(json!({}))
                }
                "shutdown" => {
                    state = "stopped";
                    Ok(json!({}))
                }
                "transfer.begin" => Ok(json!({"transfer": "t1"})),
                "program.install" => {
                    installed = true;
                    Ok(json!({}))
                }
                "cycle.step" => {
                    let time = params["time_ns"].as_u64().unwrap_or_default();
                    let cold = params["restart"] == "cold";
                    if cold && installed {
                        installed = false;
                        last = None;
                    }
                    if last.is_some_and(|last| time < last) {
                        Err("time before the last step")
                    } else {
                        last = Some(time);
                        since_cold = if cold { 1 } else { since_cold + 1 };
                        recorded.lock().expect("the steps").push(time);
                        Ok(json!({
                            "errors": [],
                            "values": {trust_runtime::native::TEST_FINISHED_GLOBAL: {"value": since_cold >= 6}},
                        }))
                    }
                }
                _ => Ok(json!({})),
            };
            // A device writes `id` first, and the link takes a reply by its leading `{"id"`.
            let id = &request["id"];
            let reply = match result {
                Ok(result) => format!(r#"{{"id":{id},"ok":true,"result":{result}}}"#),
                Err(error) => format!(r#"{{"id":{id},"ok":false,"error":"{error}"}}"#),
            };
            if writeln!(replies, "{reply}").is_err() {
                break;
            }
        }
    });
    (port, steps)
}

/// Specification 11 §6.8.1 and specification 22 ("Multi-cycle tests"), found on the C6 on 8 November: the stepped scans
/// keep one logical time across the tests, so after a multi-cycle test the next test starts where its last step was, not at
/// zero. Worked by hand for a multi-cycle test that finishes in its sixth cycle of 10 ms and a single-cycle test after it:
/// the steps run at 0, 10, 20, 30, 40 and 50 ms, and the single-cycle test's step at 50 ms; both pass, and the multi-cycle
/// test reports its own six cycles and 50 ms. Catches the single-cycle test stepped at zero after the multi-cycle test's
/// 50 ms, which the device refused with `time before the last step`.
#[test]
fn on_a_device_each_test_starts_where_the_previous_test_left_the_stepped_time() {
    let project = trust_test_support::unique_temp_path(&format!(
        "trust-dev-device-time-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(
        project.join("src/tests.st"),
        "TEST_PROGRAM Finishes\nVAR\n    Cycles : INT;\nEND_VAR\nCycles := Cycles + 1;\nIF Cycles >= 6 THEN\n    \
         TEST_FINISHED();\nEND_IF\nEND_TEST_PROGRAM\n\nTEST_PROGRAM RunsOnce\nEND_TEST_PROGRAM\n",
    )
    .expect("write the tests");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let (port, steps) = stepping_device();
    let results = execute_native_tests_on_device(
        &project,
        &discovered,
        &discovered,
        5,
        &port,
        "esp32c6",
        None,
    )
    .expect("the run on the device");
    let _ = std::fs::remove_dir_all(&project);

    let outcomes: Vec<_> = results
        .iter()
        .map(|result| (result.case.name.to_string(), outcome(result)))
        .collect();
    assert_eq!(
        outcomes,
        [
            (
                "Finishes".to_string(),
                (TestOutcome::Passed, String::new(), Some((6, 50)))
            ),
            (
                "RunsOnce".to_string(),
                (TestOutcome::Passed, String::new(), None)
            ),
        ]
    );
    let millis: Vec<u64> = steps
        .lock()
        .expect("the steps")
        .iter()
        .map(|time| time / 1_000_000)
        .collect();
    assert_eq!(millis, [0, 10, 20, 30, 40, 50, 50]);
}

/// A plant that doubles the test's command and feeds it back (specification 46, `static_gain` with `gain = 2.0`), coupled by
/// the test program's variables.
const DOUBLING_PLANT: &str = r#"
[simulation]
enabled = false
plant_step_ms = 10

[[plants]]
id = "doubler"
model = "static_gain"
command_sources = ["PlantInTheLoop.cmd"]
feedback_targets = ["PlantInTheLoop.fb"]
[plants.parameters]
gain = 2.0
"#;

/// Specification 22, "A plant in the loop", worked from specification 46 §2's order: before each cycle the plant publishes the
/// feedback of its last advance, and after each cycle it reads the command and advances to the cycle's time. So the feedback
/// reads 0 in cycle 1, before the plant has seen a command; after one plant step on the command 2.5 it reads 2 × 2.5 = 5.0 in
/// cycle 3, when the test commands 1.0; and it reads 2.0 in cycle 5, which finishes the test at t = 40 ms. The file's
/// `enabled = false` does not stop it, as the test asks for the simulation. Catches a test run without its plant (the feedback
/// stays 0), a plant advanced by the wall clock or not at the cycles' times, a plant that does not follow a later command
/// (5.0 in cycle 5), and a file's `enabled` that switches a test's plant off.
#[test]
fn a_test_with_test_simulation_runs_the_projects_plants_in_lockstep() {
    let source = "{attribute 'test_simulation'}\n\
         TEST_PROGRAM PlantInTheLoop\n\
         VAR\n    cmd : LREAL;\n    fb : LREAL;\n    n : INT;\nEND_VAR\n\
         n := n + 1;\n\
         IF n = 1 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#0.0, ACTUAL := fb);\n    cmd := LREAL#2.5;\nEND_IF\n\
         IF n = 3 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#5.0, ACTUAL := fb);\n    cmd := LREAL#1.0;\nEND_IF\n\
         IF n = 5 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#2.0, ACTUAL := fb);\n    TEST_FINISHED();\nEND_IF\n\
         END_TEST_PROGRAM\n";
    let results =
        results_with_simulation("plant", source, Some(DOUBLING_PLANT), &["PlantInTheLoop"]);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((5, 40))),
        "{:?}",
        results[0].message
    );
}

/// A device for the tests that steps a real program: `image`, the tests built for this computer as the runner builds them,
/// stepped by the device's own handler of `cycle.step` (specification 11 §6.8.1), so that a run on it steps the program as a
/// device steps it. It answers an install, a start and a stop as a device does, and steps `image` in place of the program the
/// runner installs.
fn stepping_runtime_device(image: Vec<u8>) -> String {
    use std::io::{BufRead, Write as _};
    use std::sync::Mutex;
    use trust_runtime_services::cycle_step::{self, StepOutcome, StepRefusal, StepRequest};
    use trust_runtime_services::ports::ResourcePort;
    use trust_runtime_services::resource::commands::CommandSendError;

    struct Device {
        program: Mutex<trust_runtime::CompiledRuntime>,
        last: Mutex<Option<trust_runtime::value::Duration>>,
    }
    impl ResourcePort for Device {
        fn resource_state(&self) -> trust_runtime_core::scheduler::ResourceState {
            trust_runtime_core::scheduler::ResourceState::Stopped
        }
        fn last_error(&self) -> Option<RuntimeError> {
            None
        }
        fn is_hung(&self) -> bool {
            false
        }
        fn pause_resource(&self) -> Result<(), CommandSendError> {
            Ok(())
        }
        fn resume_resource(&self) -> Result<(), CommandSendError> {
            Ok(())
        }
        fn debug_control_mode(&self) -> bool {
            true
        }
        fn debugger_pause(&self) {}
        fn debugger_continue(&self) {}
        fn update_pending_restart(
            &self,
            _update: impl FnOnce(&mut Option<trust_runtime_core::retain::RestartMode>),
        ) {
        }
        fn starts_when_stopped(&self) -> bool {
            false
        }
        fn step_cycle(&self, request: &StepRequest) -> Result<StepOutcome, StepRefusal> {
            let mut last = self.last.lock().expect("the last step");
            let restart_at = cycle_step::restart_time(request, *last)?;
            let outcome = self
                .program
                .lock()
                .expect("the program")
                .step(request, restart_at)?;
            *last = Some(request.time);
            Ok(outcome)
        }
    }

    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind the device's bridge");
    let port = format!(
        "tcp://{}",
        listener.local_addr().expect("the bridge's address")
    );
    std::thread::spawn(move || {
        let device = Device {
            program: Mutex::new(
                trust_runtime::CompiledRuntime::load(image, "RESOURCE").expect("the image loads"),
            ),
            last: Mutex::new(None),
        };
        let (stream, _) = listener.accept().expect("the link connects");
        let mut replies = stream.try_clone().expect("the bridge's stream");
        let mut state = "stopped";
        for line in std::io::BufReader::new(stream).lines() {
            let Ok(line) = line else { break };
            let request: serde_json::Value = serde_json::from_str(&line).expect("a JSON request");
            let id = request["id"].as_u64().unwrap_or_default();
            // A device writes `id` first, as the step's response does, and the link takes a reply by its leading `{"id"`.
            let reply = match request["type"].as_str().unwrap_or_default() {
                "cycle.step" => serde_json::to_string(&cycle_step::request(
                    id,
                    Some(&request["params"]),
                    &device,
                ))
                .expect("a reply"),
                kind => {
                    let result = match kind {
                        "status" => json!({"state": state, "version": env!("CARGO_PKG_VERSION")}),
                        "restart" => {
                            state = "running";
                            json!({})
                        }
                        "shutdown" => {
                            state = "stopped";
                            json!({})
                        }
                        "transfer.begin" => json!({"transfer": "t1"}),
                        _ => json!({}),
                    };
                    format!(r#"{{"id":{id},"ok":true,"result":{result}}}"#)
                }
            };
            if writeln!(replies, "{reply}").is_err() {
                break;
            }
        }
    });
    port
}

/// The results of running every test of a project whose only source is `source`, with `simulation` as its
/// `simulation.toml`, on a device that steps a real program (`stepping_runtime_device`).
fn results_on_a_stepping_device(label: &str, source: &str, simulation: &str) -> Vec<ExecutedTest> {
    let project = trust_test_support::unique_temp_path(&format!(
        "trust-dev-device-{label}-{}",
        std::process::id()
    ));
    std::fs::create_dir_all(project.join("src")).expect("project folder");
    std::fs::write(project.join("src/tests.st"), source).expect("write the tests");
    std::fs::write(project.join("simulation.toml"), simulation).expect("write the simulation");
    let sources =
        load_sources(&resolve_sources_root(&project, None).expect("sources")).expect("load");
    let discovered = discover_tests(&sources);
    let toolchain = trust_runtime::native::Toolchain::find().expect("the toolchain");
    let host = trust_runtime::native::Target::this_cpu().expect("this computer's target");
    let image = build_test_image(&project, &discovered, host, &toolchain)
        .expect("the tests build for this computer")
        .image;
    let port = stepping_runtime_device(image);
    let results =
        execute_native_tests_on_device(&project, &discovered, &discovered, 5, &port, "host", None)
            .expect("the run on the device");
    let _ = std::fs::remove_dir_all(&project);
    results
}

/// A plant wired to the process image: a static gain of 3 from the test's command at `%QL0` to its
/// feedback at `%IL0`.
const ADDRESS_PLANT: &str = r#"
[simulation]
enabled = false
plant_step_ms = 10

[[plants]]
id = "tripler"
model = "static_gain"
command_sources = ["%QL0"]
feedback_targets = ["%IL0"]
[plants.parameters]
gain = 3.0
"#;

/// A test whose command and feedback are located variables at `%QL0` and `%IL0`.
const ON_ADDRESSES: &str = "{attribute 'test_simulation'}\n\
     TEST_PROGRAM PlantOnAddresses\n\
     VAR\n    cmd AT %QL0 : LREAL;\n    fb AT %IL0 : LREAL;\n    n : INT;\nEND_VAR\n\
     n := n + 1;\n\
     IF n = 1 THEN\n    cmd := LREAL#3.0;\nEND_IF\n\
     IF n = 3 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#9.0, ACTUAL := fb);\n    TEST_FINISHED();\nEND_IF\n\
     END_TEST_PROGRAM\n";

/// Specification 22, "A plant in the loop", with a plant wired to process-image addresses: each cycle
/// samples the test's inputs before its body and publishes its outputs after it, as a scan does
/// (specification 11 §6.8.1, a step's `io`). Worked from specification 46 §2's order: the command 3.0
/// that cycle 1 publishes at `%QL0` reaches the plant after it, the plant advances one step at 10 ms to
/// 3 × 3.0 = 9.0, which reaches `%IL0` before cycle 3, so the test passes in 3 cycles at 20 ms. Catches
/// a test cycle that runs the body alone, as on the C6 on 8 November: the feedback read 0.0 in cycle 3.
#[test]
fn a_test_with_test_simulation_reaches_a_plant_wired_to_addresses() {
    let results = results_with_simulation(
        "addresses",
        ON_ADDRESSES,
        Some(ADDRESS_PLANT),
        &["PlantOnAddresses"],
    );
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((3, 20))),
        "{:?}",
        results[0].message
    );
}

/// The same plant on a device: the step carries the feedback the plant writes at `%IL0` as a direct
/// input and the device samples it into the test, and the reply reads the command the test published at
/// `%QL0` back; 9.0 in cycle 3 as on the computer. Catches a device step without `io`.
#[test]
fn a_test_with_test_simulation_reaches_a_plant_wired_to_addresses_on_a_device() {
    let results = results_on_a_stepping_device("addresses", ON_ADDRESSES, ADDRESS_PLANT);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((3, 20))),
        "{:?}",
        results[0].message
    );
}

/// Specification 22, "A plant in the loop", on a device: the plants run on this computer and the test on the device, which
/// the step carries the plants' feedback to and reads the test's command back from, so the run is the run on this computer:
/// worked as there, the feedback reads 0 in cycle 1, 5.0 in cycle 3 and 2.0 in cycle 5, which finishes the test at t = 40 ms.
/// Catches a device run that refuses the simulation (as T1b's first version did), a plant that never sees the device's
/// command (the feedback stays 0), and feedback that reaches the device a cycle late or is lost to the cold restart of the
/// first step (cycle 3 would read 0).
#[test]
fn a_test_with_test_simulation_runs_on_a_device_in_lockstep_with_its_plants() {
    let source = "{attribute 'test_simulation'}\n\
         TEST_PROGRAM PlantInTheLoop\n\
         VAR\n    cmd : LREAL;\n    fb : LREAL;\n    n : INT;\nEND_VAR\n\
         n := n + 1;\n\
         IF n = 1 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#0.0, ACTUAL := fb);\n    cmd := LREAL#2.5;\nEND_IF\n\
         IF n = 3 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#5.0, ACTUAL := fb);\n    cmd := LREAL#1.0;\nEND_IF\n\
         IF n = 5 THEN\n    ASSERT_EQUAL(EXPECTED := LREAL#2.0, ACTUAL := fb);\n    TEST_FINISHED();\nEND_IF\n\
         END_TEST_PROGRAM\n";
    let results = results_on_a_stepping_device("plant", source, DOUBLING_PLANT);
    assert_eq!(
        outcome(&results[0]),
        (TestOutcome::Passed, String::new(), Some((5, 40))),
        "{:?}",
        results[0].message
    );
}

/// Specification 22, "A plant in the loop": a test that asks for the project's simulation in a project without
/// `simulation.toml` ends as an error that says so, and runs no cycle. Catches such a test run silently without a plant.
#[test]
fn a_test_with_test_simulation_in_a_project_without_one_is_an_error() {
    let source =
        "{attribute 'test_simulation'}\nTEST_PROGRAM NoPlant\nTEST_FINISHED();\nEND_TEST_PROGRAM\n";
    let results = results_with_simulation("no-plant", source, None, &["NoPlant"]);
    assert_eq!(
        outcome(&results[0]),
        (
            TestOutcome::Error,
            "the test runs the project's simulation ({attribute 'test_simulation'}), but the project has no simulation.toml"
                .to_string(),
            None
        )
    );
}
