//! trust-sim's replay of a cycle trace against the trust-runtime this build made (specification 48
//! RU-60, "In `cargo test`"): a trace that a simulator of a project recorded matches a simulator of
//! the same project, and fails with the report against a changed one.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod simulator_support;

use std::path::PathBuf;
use std::time::Duration;

use serde_json::json;
use trust_runtime_core::trace::{TraceEnd, TraceOrigin};

use simulator_support::{project_with, trust_runtime, Simulator, IO_TOML, RUNTIME_TOML};

/// A program that counts its activations in a 10 ms task and writes the count times `factor` to
/// the located output `level` at `%QW0`.
fn counting_st(factor: u32) -> String {
    format!(
        "
VAR_GLOBAL
    level AT %QW0 : INT;
END_VAR
PROGRAM Main
VAR
    count : INT := 0;
END_VAR
count := count + 1;
level := count * {factor};
END_PROGRAM
CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
"
    )
}

/// This build's trust-runtime, which Cargo names to its integration tests when they run.
fn runtime() -> PathBuf {
    std::env::var_os("CARGO_BIN_EXE_trust-runtime")
        .expect("Cargo provides trust-runtime to its integration tests")
        .into()
}

#[test]
fn a_trace_matches_its_project_and_fails_with_the_report_against_a_changed_one(
) -> trust_sim::Result<()> {
    // Specification 48 RU-60, "In `cargo test`": `trust_sim::Replay::open(path)?.run(&project)?`
    // returns `Ok(())` when the outputs match, and otherwise an error with the report. Worked by
    // hand: `level` = `count` * 10 is 10, 20 and 30 in the 3 recorded cycles at 0, 10 and 20 ms,
    // and the same project replays them. A project that writes `count` * 20 has 20 where the trace
    // has 10 at cycle 0, at 0 ms, and the 2 cycles after it differ too: a changed program, laid
    // out as the recorded one, so without notes. A simulator's own replay of the same project
    // reports the 3 cycles of the same program. A file that does not exist cannot be read. Catches
    // a replay that passes a build which differs, an error without its report, and a missing file
    // taken for a trace.
    let recorded = project_with(&counting_st(10));
    let mut recorder = Simulator::start(&recorded);
    let started = recorder.request(1, "trace.record", json!({"cycles": 3}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(2, 3);
    let (_, file) = recorder.trace(3, trace_id);
    let path = recorded.join("commissioning.trusttrace");
    std::fs::write(&path, &file).expect("write the trace");

    let replay = trust_sim::Replay::open(&path)?;
    let same = trust_sim::Project::open_with_runtime(&recorded, runtime())?;
    replay.run(&same)?;
    assert_eq!(
        same.simulator()?.replay(&file)?.to_string(),
        "Replayed 3 cycles of the same program: every cycle's outputs match the trace.\n"
    );

    let changed = project_with(&counting_st(20));
    assert_eq!(
        replay.run(&trust_sim::Project::open_with_runtime(&changed, runtime())?),
        Err(trust_sim::Error::Replay {
            trace: path,
            report: "Replayed 3 cycles of a changed program.\n\
                     The first difference is at cycle 0, at the logical time 0 ms:\n\
                     \x20 level (%QW0): the trace has 10, the build 20\n\
                     2 later cycles differ too.\n"
                .to_string(),
        })
    );

    assert!(matches!(
        trust_sim::Replay::open(recorded.join("missing.trusttrace")),
        Err(trust_sim::Error::Trace { .. })
    ));
    Ok(())
}

/// A program that counts its activations in a 10 ms task, writes the count times 10 to the located
/// output `level` at `%QW0`, and divides by zero in its 5th activation.
const FAULTING_ST: &str = "
VAR_GLOBAL
    level AT %QW0 : INT;
END_VAR
PROGRAM Main
VAR
    count : INT := 0;
    divisor : INT := 1;
    quotient : INT;
END_VAR
count := count + 1;
level := count * 10;
IF count = 5 THEN
    divisor := 0;
END_IF;
quotient := level / divisor;
END_PROGRAM
CONFIGURATION Plant
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM P1 WITH Fast : Main;
END_RESOURCE
END_CONFIGURATION
";

/// A built project of [`FAULTING_ST`] whose runtime keeps a flight recorder of 3 cycles.
fn flight_recorded_project() -> PathBuf {
    let project = trust_test_support::unique_temp_path("trust-sim-fault-trace");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    let runtime_toml = format!("{RUNTIME_TOML}[runtime.trace]\nflight_recorder = 3\n");
    std::fs::write(project.join("runtime.toml"), runtime_toml).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), IO_TOML).expect("write io.toml");
    std::fs::write(project.join("src/main.st"), FAULTING_ST).expect("write main.st");
    let built = trust_runtime()
        .arg("build")
        .arg("--project")
        .arg(&project)
        .output()
        .expect("run trust-runtime build");
    assert!(
        built.status.success(),
        "trust-runtime build failed:\n{}",
        String::from_utf8_lossy(&built.stderr)
    );
    project
}

#[test]
fn a_machine_tests_fault_keeps_its_trace_for_a_replay() -> trust_sim::Result<()> {
    // Specification 48 RU-60, "In `cargo test`", and RU-59, "The flight recorder": a machine test
    // reads the simulation's fault trace with `sim.fault_trace()?`, none before a fault, and a test
    // that faults on purpose replays the cycles before the fault. Worked by hand for a flight
    // recorder of 3 cycles and a program that divides by zero in its 5th cycle: before the fault
    // there is no trace; running 10 cycles stops at the fault at 40 ms; the first recording holds
    // 0, 10 and 20 and hands over, the second holds 30 and the faulted 40, so the fault's trace
    // joins them: 5 cycles at 0 to 40 ms, from the flight recorder, ending `faulted`. A new
    // simulator of the project replays it with every cycle's outputs and the fault matching.
    // Catches a fault's trace given before a fault, one that is not the flight recorder's or lacks
    // the faulted cycle, and one that does not replay.
    let project = flight_recorded_project();
    let mut sim = trust_sim::Project::open_with_runtime(&project, runtime())?.simulator()?;
    assert_eq!(sim.fault_trace()?, None);
    let faulted = sim.run_cycles(10);
    assert!(
        matches!(faulted, Err(trust_sim::Error::Fault { time, .. }) if time == Duration::from_millis(40)),
        "{faulted:?}"
    );
    let file = sim.fault_trace()?.expect("the fault's trace");
    let trace = trust_runtime_core::trace::decode(&file)
        .expect("the fault's trace decodes")
        .trace;
    assert_eq!(trace.meta.origin, TraceOrigin::FlightRecorder);
    assert_eq!(trace.meta.end, TraceEnd::Faulted);
    let times: Vec<i64> = trace
        .cycles
        .iter()
        .map(|cycle| cycle.time_ns / 1_000_000)
        .collect();
    assert_eq!(times, [0, 10, 20, 30, 40]);
    assert!(trace.cycles[4].fault.is_some());

    let replayed = trust_sim::Project::open_with_runtime(&project, runtime())?
        .simulator()?
        .replay(&file)?;
    assert!(replayed.matched, "{replayed}");
    Ok(())
}
