//! A Rust program runs in a configuration as a Structured Text program does (specification 48
//! RU-35; RU-47 for its budget): its program connections bind its inputs and outputs around its
//! Rust body, its retained variable lies in the retained area and keeps its value through a warm
//! restart, and its budget counts against the task that runs it.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources, as a release's bundle is until specification 48 §16, refuses the
//! build with that reason.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::rust_pous::Written;
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};

/// The root of a crate whose program the configuration runs: a configuration binds each program the
/// crate declares, as it binds each of Structured Text's.
const ROOT: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;
";

/// A Rust program that follows its target.
const AXIS_CONTROLLER: &str = "
/// Moves its position half the way to its target at each run, and counts its runs.
#[trust_program(name = \"AXIS_CONTROLLER\")]
#[budget(\"2us\")]
pub struct AxisController {
    #[input]
    pub target: f64,
    #[output]
    pub command: f64,
    #[retain]
    pub runs: u32,
    position: f64,
}

impl Body for AxisController {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.runs = checked::add(cx, self.runs, 1)?;
        self.position += (self.target - self.position) * 0.5;
        self.command = self.position;
        Ok(())
    }
}
";

/// A Rust program whose budget is twice its task's interval.
const SLOW_AXIS: &str = "
/// Counts its runs.
#[trust_program(name = \"SLOW_AXIS\")]
#[budget(\"20ms\")]
pub struct SlowAxis {
    #[output]
    pub ticks: i16,
}

impl Body for SlowAxis {
    fn body(&mut self, cx: &mut Cx) -> Result<(), Fault> {
        self.ticks = checked::add(cx, self.ticks, 1)?;
        Ok(())
    }
}
";

/// The configuration that runs AXIS_CONTROLLER every 10 ms, its target and command connected to
/// globals.
const AXIS: &str = "CONFIGURATION Cell
VAR_GLOBAL
    goal : LREAL := 8.0;
    drive : LREAL;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Axis WITH Fast : AXIS_CONTROLLER(target := goal, command => drive);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_configuration_runs_a_rust_program_with_its_connections() {
    // RU-35: the task runs Axis at 0, 10 and 20 ms (its start and every 10 ms, specification 10
    // §4.3). Its input
    // `target` is connected to the global goal, 8; its position moves half the way at each run, to
    // 4, 6 and 7, and its output `command`, connected to the global drive, follows it; its retained
    // `runs` counts 3. A warm restart keeps `runs`, and starts `position` and drive again at 0, so
    // the next run, at the restart's start, moves the position to 4 and counts 4. Catches a program whose
    // connections are not made around its Rust body, a retained variable read or written in the
    // instance's area, and a warm restart that keeps or loses the wrong variables.
    let root = custom_project(
        "rust-programs",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{AXIS_CONTROLLER}")),
        ],
        AXIS,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("AXIS.COMMAND"), Some(Value::LReal(7.0)));
    assert_eq!(program.value("DRIVE"), Some(Value::LReal(7.0)));
    assert_eq!(program.value("AXIS.RUNS"), Some(Value::UDInt(3)));
    program
        .warm_start(Duration::ZERO)
        .expect("the warm restart");
    assert_eq!(program.value("AXIS.RUNS"), Some(Value::UDInt(3)));
    assert_eq!(program.value("DRIVE"), Some(Value::LReal(0.0)));
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("AXIS.COMMAND"), Some(Value::LReal(4.0)));
    assert_eq!(program.value("DRIVE"), Some(Value::LReal(4.0)));
    assert_eq!(program.value("AXIS.RUNS"), Some(Value::UDInt(4)));
}

#[test]
fn a_rust_programs_budget_counts_against_its_task() {
    // RU-47: the task Fast runs SLOW_AXIS once a scan, and its budget of 20 ms is more than the
    // task's interval of 10 ms: a warning in the default `development` profile, and the image
    // still builds. Catches a Rust program whose own budget the admission leaves out because no
    // Structured Text calls it.
    let root = custom_project(
        "rust-programs-budget",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{SLOW_AXIS}")),
        ],
        "CONFIGURATION Cell
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Slow WITH Fast : SLOW_AXIS;
END_RESOURCE
END_CONFIGURATION
",
    );
    let Some((built, _)) = built(&root) else {
        return;
    };
    assert_eq!(
        built.warnings,
        ["the task Fast may spend 20 ms per scan in Rust POUs (SLOW_AXIS 1 × 20 ms), more than its \
          interval 10 ms (specification 48 RU-47)"]
    );
}

#[test]
fn a_rust_first_project_runs_the_configuration_of_its_trust_toml() {
    // RU-65 to RU-67: a Rust-first project is its crate, with `trust.toml` and no hand-written
    // Structured Text: `trust-dev rust declarations` writes its declarations and its configuration
    // into `src/generated/trust/`, and the build runs the program instance `trust.toml` declares on
    // its task. Axis runs at 0, 10 and 20 ms toward the target 0 it starts at and counts 3 runs.
    // Catches a crate in the project's own folder not found or not compiled, and a configuration
    // the build does not read.
    let root = trust_test_support::unique_temp_path("rust-first-project");
    let files = [
        ("Cargo.toml", manifest("")),
        ("src/lib.rs", format!("{ROOT}{AXIS_CONTROLLER}")),
        (
            "trust.toml",
            "[rust]\ncrate = \".\"\n\n[resource]\nname = \"Cell\"\n\n[tasks.Fast]\n\
             interval = \"10ms\"\npriority = 1\n\n[programs.Axis]\ntype = \"AXIS_CONTROLLER\"\n\
             task = \"Fast\"\n"
                .to_string(),
        ),
    ];
    for (path, text) in &files {
        let file = root.join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    let written = trust_runtime::native::rust_pous::write_generated(&root);
    assert!(
        matches!(
            written,
            Ok(trust_runtime::native::rust_pous::Generated {
                declarations: Written::Wrote(1),
                configuration: Some(Written::Wrote(1)),
                ..
            })
        ),
        "{written:?}"
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): three activations, at 0, 10 and
    // 20 ms.
    for time in [0, 10, 20] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("AXIS.RUNS"), Some(Value::UDInt(3)));
}
