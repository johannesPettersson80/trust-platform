//! The CODESYS toolkit's S9 end to end (protocol annex E2.3; specification 35, B3 to B6): a loop that a CODESYS project keeps in a
//! GVL goes through Identify, Tune, Validate, a transfer to Auto and Apply on a real runtime, through the command line of
//! `trust-control-studio`.
//!
//! The project is lesson 30's heater (`common::lesson_30`) in CODESYS layout. Seven variables of its program `Main`, the mode, the
//! manual output, the setpoint, the two measurements and the two outputs, move into the GVL `GVL_IO` (`src/GVL_IO.st`) at the
//! addresses the configuration gave them, and the program and the loop file name them there. The `Control.PID`, its parameter record
//! and its states stay in the program instance `P1`. Identify's step therefore writes a GVL member (G97c), and the transfer to Auto
//! and Apply's own return to Auto write the GVL's mode, setpoint and manual output beside `P1`'s states (G97b); `P1`, in task
//! `MainTask`, is the only program that writes the GVL, so the task owns each global the loop writes. The runtime runs at ten times
//! real time, as the Apply journey does; the run takes about a minute.
//!
//! Unix only, as the other real-runtime journeys are: the runtime's endpoint is a Unix socket.
#![cfg(unix)]

mod common;

use std::fs;
use std::path::{Path, PathBuf};

use common::engine::{Findings, Lab};
use common::lesson_30::{
    transfer, wait_for_operating_point, LESSON_30, LIMIT, OPERATING_POINT, RETURNED_WITHIN,
};
use common::RemoveDir;

/// The loop's file in the copy.
const LOOP_FILE: &str = "control-loop.json";
/// The speed of the simulation, ten times real time.
const TIME_SCALE: u32 = 10;
/// A control endpoint that nothing listens on: Tune must finish without reaching it (B4.1).
const UNREACHABLE: &str = "tcp://127.0.0.1:1";

/// The variables of lesson 30's program `Main` that a CODESYS project keeps in a GVL, each with its name in `GVL_IO`.
const MOVED: [(&str, &str); 7] = [
    ("AutoMode", "bAuto"),
    ("ManualOutput", "lrManualOutput"),
    ("SetpointC", "lrSetpoint"),
    ("WaterInputC", "lrWater"),
    ("TemperatureC", "lrTemperature"),
    ("AppliedPowerPct", "lrPower"),
    ("DisturbanceKw", "lrLoadKw"),
];

/// The GVL: the moved variables with the initial values `Main` gave them, at the addresses its configuration gave them.
const GVL_IO: &str = "NAMESPACE GVL_IO
VAR_GLOBAL
    bAuto : BOOL := FALSE;
    lrManualOutput AT %ML72 : LREAL := 75.0;
    lrSetpoint AT %ML0 : LREAL := 35.0;
    lrWater AT %IL0 : LREAL := 35.0;
    lrTemperature AT %IL8 : LREAL := 35.0;
    lrPower AT %QL0 : LREAL := 75.0;
    lrLoadKw AT %QL8 : LREAL := 0.0;
END_VAR
END_NAMESPACE
";

/// `text` with each use of the identifier `name` replaced by `replacement`, except where it is part of a longer name or a member of
/// something else (`Input.ManualOutput` stays).
fn renamed(text: &str, name: &str, replacement: &str) -> String {
    let identifier = |character: Option<char>| {
        character.is_some_and(|character| character.is_alphanumeric() || character == '_')
    };
    let mut out = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(found) = rest.find(name) {
        let (before, after) = (&rest[..found], &rest[found + name.len()..]);
        let previous = before.chars().next_back();
        out.push_str(before);
        if identifier(previous) || previous == Some('.') || identifier(after.chars().next()) {
            out.push_str(name);
        } else {
            out.push_str(replacement);
        }
        rest = after;
    }
    out.push_str(rest);
    out
}

/// A copy of lesson 30 in CODESYS layout in `folder`: the moved variables leave `Main`'s declarations and the configuration's
/// `VAR_CONFIG`, `src/GVL_IO.st` declares them, and the program and the loop file name them in the GVL.
fn codesys_copy(folder: &Path) -> PathBuf {
    let project = folder.join("lesson-30-gvl");
    common::copy_dir(Path::new(LESSON_30), &project);
    let main = project.join("src/Main.st");
    let mut text = fs::read_to_string(&main).expect("the lesson's Main.st");
    let moved = |line: &str| {
        let line = line.trim_start();
        MOVED.iter().any(|(name, _)| {
            line.starts_with(&format!("{name} : ")) || line.starts_with(&format!("P1.{name} AT "))
        })
    };
    let mut kept = String::with_capacity(text.len());
    for line in text.lines().filter(|line| !moved(line)) {
        kept.push_str(line);
        kept.push('\n');
    }
    text = kept;
    for (name, global) in MOVED {
        text = renamed(&text, name, &format!("GVL_IO.{global}"));
    }
    fs::write(&main, text).expect("write Main.st");
    fs::write(project.join("src/GVL_IO.st"), GVL_IO).expect("write the GVL");
    let loop_file = project.join(LOOP_FILE);
    let mut loop_text = fs::read_to_string(&loop_file).expect("the lesson's loop file");
    for (name, global) in MOVED {
        loop_text = loop_text.replace(&format!("\"P1.{name}\""), &format!("\"GVL_IO.{global}\""));
    }
    fs::write(&loop_file, loop_text).expect("write the loop file");
    project
}

/// Worked from the copy: the loop's mode, setpoint, manual output and output are `GVL_IO`'s, and only `P1`, which `MainTask` runs,
/// writes them, so the transfers and the step that the journey makes are the task's own (annex E2.3). Identify, Tune and Validate
/// give a candidate as on the lesson itself; the transfer to Auto is confirmed; Apply passes, its record holds the candidate's
/// values and the loop stays in Auto. Catches a loop kept in a GVL refused at any of these steps, as the toolkit found it before
/// 0.35.0 ("transfer members must belong to one compiled program").
#[test]
fn a_loop_kept_in_a_gvl_is_identified_tuned_validated_and_applied() {
    let folder = trust_test_support::unique_temp_path("trust-cs-gvl");
    let _ = fs::remove_dir_all(&folder);
    let _remove = RemoveDir(folder.clone());
    let copy = codesys_copy(&folder);
    let lab = Lab::start(
        "gvl-30",
        copy.to_str().expect("a UTF-8 path"),
        LOOP_FILE,
        TIME_SCALE,
    );
    assert_eq!(
        lab.loop_definition
            .signals
            .setpoint
            .as_ref()
            .map(|signal| signal.symbol.as_str()),
        Some("GVL_IO.lrSetpoint"),
        "the copy's loop names the GVL"
    );
    let mut findings = Findings::default();

    let (program, status) = lab.run("identify", "identify", &["--json"], true, LIMIT);
    findings.require(
        status.code() == Some(0) && program.answer().get("result").is_some(),
        format!(
            "identify finishes with its result (B3):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "tune",
        "tune",
        &["--method", "simc", "--endpoint", UNREACHABLE, "--json"],
        false,
        LIMIT,
    );
    findings.require(
        status.code() == Some(0) && program.answer()["result"]["candidate"]["applied"] == false,
        format!(
            "tune designs an unapplied candidate (B4.1):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run("validate", "validate", &["--json"], true, LIMIT);
    findings.require(
        program.answer()["result"]["applyAvailable"] == true,
        format!(
            "validate leaves Apply available (B5.4):\n{}",
            program.report(status)
        ),
    );

    findings.check(
        lab.is_auto() == Some(false),
        "the program starts the loop in Manual (GVL_IO.bAuto is FALSE)",
    );
    let returned = wait_for_operating_point(&lab);
    findings.require(
        returned.is_some_and(|value| (value - OPERATING_POINT).abs() <= RETURNED_WITHIN),
        format!("the water returns to the operating point of {OPERATING_POINT} °C (B6.2): {returned:?} °C"),
    );
    // The transfer writes GVL_IO's mode, setpoint and manual output with P1's states; `transfer` asserts that the runtime confirms it.
    transfer(&lab, false);
    findings.require(
        lab.is_auto() == Some(true),
        "the transfer moved the loop to Auto",
    );

    let (program, status) = lab.run("apply-review", "apply", &["--json"], true, LIMIT);
    let review = program.answer()["result"].clone();
    let identity = review["identity"].as_str().unwrap_or_default().to_string();
    findings.require(
        status.code() == Some(0)
            && review["foundMode"] == "auto"
            && identity.starts_with("sha256:"),
        format!(
            "the review in Auto names its identity (§0.1b):\n{}",
            program.report(status)
        ),
    );
    let (program, status) = lab.run(
        "apply",
        "apply",
        &["--review", identity.as_str(), "--json"],
        true,
        LIMIT,
    );
    findings.require(
        status.code() == Some(0) && program.answer()["result"]["result"]["status"] == "passed",
        format!(
            "the Apply passes and exits with status 0 (B6.3):\n{}",
            program.report(status)
        ),
    );
    findings.check(
        lab.is_auto() == Some(true),
        "the loop is in Auto after Apply",
    );
    findings.conclude();
}
