//! trust-sim's replay of a cycle trace against the trust-runtime this build made (specification 48
//! RU-60, "In `cargo test`"): a trace that a simulator of a project recorded matches a simulator of
//! the same project, and fails with the report against a changed one.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

mod simulator_support;

use std::path::PathBuf;

use serde_json::json;

use simulator_support::{project_with, Simulator};

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

fn runtime() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_trust-runtime"))
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
