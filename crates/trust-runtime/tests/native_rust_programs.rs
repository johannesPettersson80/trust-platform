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

/// RU-70's palletizer, a Rust program that runs a sequence.
const PALLETIZER: &str = "
/// Grips a part when it comes, within 2 s, holds it 500 ms and lets it go.
#[trust_program(name = \"PALLETIZER\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Palletizer {
    #[input]
    pub part_present: bool,
    #[input]
    pub gripper_closed: bool,
    #[output]
    pub close_gripper: bool,
    #[output]
    pub active_step: Str<16>,
}

#[trust_sequence]
impl Palletizer {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.step(&mut self.active_step, \"WAIT_PART\");
            s.until(|me: &Self| me.part_present).await;
            s.step(&mut self.active_step, \"GRIP\");
            self.close_gripper = true;
            s.until(|me: &Self| me.gripper_closed)
                .within(Time::from_millis(s, 2_000)?)
                .await?;
            s.wait(Time::from_millis(s, 500)?).await;
            self.close_gripper = false;
        }
    }
}
";

/// The configuration that runs PALLETIZER every 10 ms, its inputs and its output connected to
/// globals.
const CELL: &str = "CONFIGURATION Line
VAR_GLOBAL
    part : BOOL;
    closed : BOOL;
    gripper : BOOL;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : PALLETIZER(part_present := part, gripper_closed := closed, close_gripper => gripper);
END_RESOURCE
END_CONFIGURATION
";

/// Where Cell's sequence waits, and its gripper output's global.
fn cell(
    program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>,
) -> (Option<Value>, Option<Value>) {
    (program.value("CELL._SEQ.POINT"), program.value("GRIPPER"))
}

#[test]
fn a_configuration_runs_a_rust_sequence_scan_by_scan() {
    // Specification 48 §15 in a project: the generated declarations declare the state `_seq` last
    // (RU-71) and the program's code passes it to the sequence at each activation. Activations every
    // 10 ms: at 0 ms the sequence waits for a part, point 1; the part comes before 10 ms, so at
    // 10 ms it grips, point 2; the gripper closes before 20 ms, so the hold of 500 ms, point 3,
    // begins at 20 ms, lasts through 510 ms and is over at 520 ms, when the gripper lets go and the
    // sequence waits for a part again, point 1; at 530 ms the part is still there and it grips.
    // A warm restart starts the sequence at its top (RU-73): its first activation waits for a part,
    // point 1, with the gripper open. Catches a state the program's code does not pass, one that
    // does not survive from one scan to the next, a hold that ends a cycle early or late, and a
    // restart that resumes a wait.
    let root = custom_project(
        "rust-sequences",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{PALLETIZER}")),
        ],
        CELL,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let scan = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                time: i64| {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    };
    let point = |number: u16| Some(Value::UInt(number));
    let gripper = |closed: bool| Some(Value::Bool(closed));
    scan(&mut program, 0);
    assert_eq!(cell(&program), (point(1), gripper(false)), "at 0 ms");
    let (part, _) = program
        .image()
        .find_symbol("PART")
        .expect("the symbol of part");
    program
        .write_symbol(part, &Value::Bool(true))
        .expect("the part comes");
    scan(&mut program, 10);
    assert_eq!(cell(&program), (point(2), gripper(true)), "at 10 ms");
    let (closed, _) = program
        .image()
        .find_symbol("CLOSED")
        .expect("the symbol of closed");
    program
        .write_symbol(closed, &Value::Bool(true))
        .expect("the gripper closes");
    for time in (20..=510).step_by(10) {
        scan(&mut program, time);
    }
    assert_eq!(cell(&program), (point(3), gripper(true)), "at 510 ms");
    scan(&mut program, 520);
    assert_eq!(cell(&program), (point(1), gripper(false)), "at 520 ms");
    scan(&mut program, 530);
    assert_eq!(cell(&program), (point(2), gripper(true)), "at 530 ms");
    program
        .warm_start(Duration::ZERO)
        .expect("the warm restart");
    scan(&mut program, 0);
    assert_eq!(
        cell(&program),
        (point(1), gripper(false)),
        "after the warm restart"
    );
}

/// A press whose sequence waits for both guards, each a pulse, with an `s.all`, and races its
/// stroke to the bottom against a stop into the local `won`.
const PRESS: &str = "
/// Presses once both guards have closed, and ends on the bottom or a stop.
#[trust_program(name = \"PRESS\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Press {
    #[input]
    pub left_closed: bool,
    #[input]
    pub right_closed: bool,
    #[input]
    pub at_bottom: bool,
    #[input]
    pub stop_pressed: bool,
    #[output]
    pub pressing: bool,
    #[output]
    pub ended: u16,
}

#[trust_sequence]
impl Press {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(
                s.until(|me: &Self| me.left_closed),
                s.until(|me: &Self| me.right_closed),
            )
            .await;
            self.pressing = true;
            let won: u16 = s
                .race(
                    s.until(|me: &Self| me.at_bottom),
                    s.until(|me: &Self| me.stop_pressed),
                )
                .await;
            self.pressing = false;
            self.ended = won;
        }
    }
}
";

/// The configuration that runs PRESS every 10 ms, its inputs and outputs connected to globals.
const PRESS_LINE: &str = "CONFIGURATION Line
VAR_GLOBAL
    left_guard : BOOL;
    right_guard : BOOL;
    bottom : BOOL;
    stop_button : BOOL;
    pressing : BOOL;
    ended : UINT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : PRESS(left_closed := left_guard, right_closed := right_guard, at_bottom := bottom, stop_pressed := stop_button, pressing => pressing, ended => ended);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_configuration_runs_a_rust_sequences_all_and_race_scan_by_scan() {
    // Specification 48 RU-71 and RU-72 in a project: the generated state PRESS_SEQ holds `done`
    // after `began` and the local `won` after it, and the program's code passes all of them.
    // Activations every 10 ms: at 0 ms the s.all, point 1, begins; the left guard pulses before
    // 10 ms, done 1, and its bit stays at 20 ms when the pulse is gone; the right guard closes
    // before 30 ms, the last member: the press presses and races, point 2, done 0 again. Bottom
    // and stop come together before 40 ms: the first member in order wins, `won` 1, the press
    // stops, `ended` 1, and the s.all begins anew, point 1; at 50 ms the right guard, still
    // closed, completes its member, done 2. Catches a state whose `done` or `won` the declaration
    // lays out apart from the macro's, a member that must hold at one activation with the others,
    // and a race that takes the last member that completed.
    let root = custom_project(
        "rust-sequence-all-race",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{PRESS}")),
        ],
        PRESS_LINE,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let scan = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                time: i64| {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    };
    let write = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                 name: &str,
                 on: bool| {
        let (symbol, _) = program
            .image()
            .find_symbol(name)
            .unwrap_or_else(|| panic!("the symbol of {name}"));
        program
            .write_symbol(symbol, &Value::Bool(on))
            .unwrap_or_else(|error| panic!("{name} written: {error:?}"));
    };
    let press =
        |program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>| {
            (
                program.value("CELL._SEQ.POINT"),
                program.value("CELL._SEQ.DONE"),
                program.value("CELL._SEQ.WON"),
                program.value("PRESSING"),
                program.value("ENDED"),
            )
        };
    let state = |point: u16, done: u16, won: u16, pressing: bool, ended: u16| {
        (
            Some(Value::UInt(point)),
            Some(Value::Word(done)),
            Some(Value::UInt(won)),
            Some(Value::Bool(pressing)),
            Some(Value::UInt(ended)),
        )
    };
    scan(&mut program, 0);
    assert_eq!(press(&program), state(1, 0, 0, false, 0), "at 0 ms");
    write(&mut program, "LEFT_GUARD", true);
    scan(&mut program, 10);
    assert_eq!(press(&program), state(1, 1, 0, false, 0), "at 10 ms");
    write(&mut program, "LEFT_GUARD", false);
    scan(&mut program, 20);
    assert_eq!(press(&program), state(1, 1, 0, false, 0), "at 20 ms");
    write(&mut program, "RIGHT_GUARD", true);
    scan(&mut program, 30);
    assert_eq!(press(&program), state(2, 0, 0, true, 0), "at 30 ms");
    write(&mut program, "BOTTOM", true);
    write(&mut program, "STOP_BUTTON", true);
    scan(&mut program, 40);
    assert_eq!(press(&program), state(1, 0, 1, false, 1), "at 40 ms");
    scan(&mut program, 50);
    assert_eq!(press(&program), state(1, 2, 1, false, 1), "at 50 ms");
}

/// A filler whose sequence fills each bottle in a child sequence, which lets the level settle in a
/// child of its own.
const FILLER: &str = "
/// Fills a bottle when it comes and lets the level settle for two cycles.
#[trust_program(name = \"FILLER\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Filler {
    #[input]
    pub bottle: bool,
    #[input]
    pub full: bool,
    #[output]
    pub valve: bool,
    #[output]
    pub filled: i16,
}

#[trust_sequence]
impl Filler {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.until(|me: &Self| me.bottle).await;
            self.fill(s).await?;
            self.filled = checked::add(s, self.filled, 1)?;
            s.next_cycle().await;
        }
    }

    async fn fill(&mut self, s: &mut Seq) -> Result<(), Fault> {
        self.valve = true;
        s.until(|me: &Self| me.full).await;
        self.valve = false;
        self.settle(s).await?;
        Ok(())
    }

    async fn settle(&mut self, s: &mut Seq) -> Result<(), Fault> {
        let mut ticks: i16 = 0;
        while ticks < 2 {
            ticks = checked::add(s, ticks, 1)?;
            s.next_cycle().await;
        }
        Ok(())
    }
}
";

/// The configuration that runs FILLER every 10 ms, its inputs and outputs connected to globals.
const BOTTLING: &str = "CONFIGURATION Line
VAR_GLOBAL
    bottle : BOOL;
    full : BOOL;
    valve : BOOL;
    filled : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : FILLER(bottle := bottle, full := full, valve => valve, filled => filled);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_configuration_runs_a_rust_sequences_children_scan_by_scan() {
    // Specification 48 RU-71 and RU-72 in a project: the generated FILLER_SEQ holds the states of
    // the children `fill` and `settle`, FILLER_SEQ_FILL and FILLER_SEQ_SETTLE, and the program's
    // code passes them all. Activations every 10 ms: at 0 ms the filler waits for a bottle, point 1;
    // the bottle comes before 10 ms, so at 10 ms the child `fill` starts at that activation, opens
    // the valve and waits for the level, its point 1, while the filler waits at it, point 2; the
    // bottle is full before 20 ms: `fill` closes the valve and starts `settle`, which counts 1 and
    // waits, so `fill` waits at its point 2; at 30 ms `settle` counts 2; at 40 ms `settle` and `fill`
    // return and the filler counts 1 and waits a cycle, point 3, its child at its top. Catches a
    // child's state the declaration lays out apart from the macro's, a child that starts an
    // activation late, and a parent that goes on an activation after its child returned.
    let root = custom_project(
        "rust-sequence-children",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{FILLER}")),
        ],
        BOTTLING,
    );
    let Some((built, device)) = built(&root) else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let scan = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                time: i64| {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    };
    let write = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                 name: &str| {
        let (symbol, _) = program
            .image()
            .find_symbol(name)
            .unwrap_or_else(|| panic!("the symbol of {name}"));
        program
            .write_symbol(symbol, &Value::Bool(true))
            .unwrap_or_else(|error| panic!("{name} written: {error:?}"));
    };
    let filler =
        |program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>| {
            (
                program.value("CELL._SEQ.POINT"),
                program.value("CELL._SEQ.FILL.POINT"),
                program.value("CELL._SEQ.SETTLE.TICKS"),
                program.value("VALVE"),
                program.value("FILLED"),
            )
        };
    let state = |point: u16, fill: u16, ticks: i16, valve: bool, filled: i16| {
        (
            Some(Value::UInt(point)),
            Some(Value::UInt(fill)),
            Some(Value::Int(ticks)),
            Some(Value::Bool(valve)),
            Some(Value::Int(filled)),
        )
    };
    scan(&mut program, 0);
    assert_eq!(filler(&program), state(1, 0, 0, false, 0), "at 0 ms");
    write(&mut program, "BOTTLE");
    scan(&mut program, 10);
    assert_eq!(filler(&program), state(2, 1, 0, true, 0), "at 10 ms");
    write(&mut program, "FULL");
    scan(&mut program, 20);
    assert_eq!(filler(&program), state(2, 2, 1, false, 0), "at 20 ms");
    scan(&mut program, 30);
    assert_eq!(filler(&program), state(2, 2, 2, false, 0), "at 30 ms");
    scan(&mut program, 40);
    assert_eq!(filler(&program), state(3, 0, 2, false, 1), "at 40 ms");
}
