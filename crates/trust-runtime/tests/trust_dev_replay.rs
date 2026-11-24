//! `trust-dev replay` against the trust-runtime this build made (specification 48 RU-60, "The
//! result"): the report and the exit code of a trace that matches, of one that differs, and of one
//! that cannot be replayed.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, and this build's
//! trust-dev, which the workspace's tests build (`cargo build -p trust-dev` builds it alone).

mod simulator_support;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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

/// This build's trust-dev, beside the test's own folder of the target.
fn trust_dev_bin() -> PathBuf {
    if let Some(path) = std::env::var_os("CARGO_BIN_EXE_trust-dev") {
        return path.into();
    }
    if let Ok(path) = std::env::var("TRUST_DEV_BIN") {
        return path.into();
    }
    let exe = std::env::current_exe().expect("the test's own path");
    let debug = exe
        .parent()
        .and_then(Path::parent)
        .expect("the target's debug folder");
    debug.join(format!("trust-dev{}", std::env::consts::EXE_SUFFIX))
}

/// `trust-dev replay <trace> --project <project>` with this build's trust-runtime.
fn trust_dev_replay(trace: &Path, project: &Path) -> Output {
    Command::new(trust_dev_bin())
        .arg("replay")
        .arg(trace)
        .arg("--project")
        .arg(project)
        .env("TRUST_RUNTIME", env!("CARGO_BIN_EXE_trust-runtime"))
        .output()
        .expect("run trust-dev replay")
}

#[test]
fn trust_dev_replay_prints_the_report_and_ends_with_0_1_or_2() {
    // Specification 48 RU-60, "The result": "A replay that matches every cycle prints the cycles it
    // ran and ends with 0"; at the first cycle whose outputs differ it reports the cycle, its
    // logical time and each output that differs, and how many later cycles differ, and ends with
    // 1; "A trace or build it cannot replay ends it with 2 and the reason". Worked by hand:
    // `level` = `count` * 10 records 10, 20 and 30 in 3 cycles at 0, 10 and 20 ms; the same project
    // matches; one that writes `count` * 20 differs at cycle 0, 10 against 20, and in the 2 cycles
    // after it; a file that is no trace is refused by the simulator. Catches a difference that ends
    // with 0, a refusal that ends with 1, and a report that is not printed.
    let recorded = project_with(&counting_st(10));
    let mut recorder = Simulator::start(&recorded);
    let started = recorder.request(1, "trace.record", json!({"cycles": 3}));
    let trace_id = started["trace"].as_u64().expect("a recording's id");
    recorder.advance(2, 3);
    let (_, file) = recorder.trace(3, trace_id);
    let trace = recorded.join("commissioning.trusttrace");
    std::fs::write(&trace, &file).expect("write the trace");
    let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();

    let same = trust_dev_replay(&trace, &recorded);
    assert_eq!(same.status.code(), Some(0), "{}", text(&same.stderr));
    assert_eq!(
        text(&same.stdout),
        "Replayed 3 cycles of the same program: every cycle's outputs match the trace.\n"
    );

    let changed = project_with(&counting_st(20));
    let differs = trust_dev_replay(&trace, &changed);
    assert_eq!(differs.status.code(), Some(1), "{}", text(&differs.stderr));
    assert_eq!(
        text(&differs.stdout),
        "Replayed 3 cycles of a changed program.\n\
         The first difference is at cycle 0, at the logical time 0 ms:\n\
         \x20 level (%QW0): the trace has 10, the build 20\n\
         2 later cycles differ too.\n"
    );

    let damaged = recorded.join("damaged.trusttrace");
    std::fs::write(&damaged, b"not a trace").expect("write the file");
    let refused = trust_dev_replay(&damaged, &recorded);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused.stdout));
    let reason = text(&refused.stderr);
    assert!(reason.contains("the trace cannot be replayed"), "{reason}");
    assert!(text(&refused.stdout).is_empty());
}
