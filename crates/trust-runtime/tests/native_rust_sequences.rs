//! Rust sequences run in a configuration (specification 48 §15): the generated declarations declare
//! each sequence's state, the program's code passes it, and the state goes on from one scan to the
//! next, through child sequences, an online change and a restart, as RU-71 to RU-73 give.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources refuses the build with that reason, and the tests stop there.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::native::change::CarriedValueEvent;

/// The root of a crate whose program the configuration runs.
const ROOT: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;
";

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

/// A stamp whose sequence waits for a part, strokes for `STROKE` milliseconds and counts it, the
/// program of the online change test.
const STAMP: &str = "
/// Strokes once for each part.
#[trust_program(name = \"STAMP\", sequence = \"run\")]
#[budget(\"80us\")]
pub struct Stamp {
    #[input]
    pub part: bool,
    #[output]
    pub down: bool,
    #[output]
    pub strokes: i16,
}

#[trust_sequence]
impl Stamp {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.until(|me: &Self| me.part).await;
            self.down = true;
            s.wait(Time::from_millis(s, STROKE)?).await;
            self.down = false;
            self.strokes = checked::add(s, self.strokes, 1)?;
        }
    }
}
";

/// The configuration that runs STAMP every 10 ms, its input and outputs connected to globals.
const STAMPING: &str = "CONFIGURATION Line
VAR_GLOBAL
    part : BOOL;
    down : BOOL;
    strokes : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : STAMP(part := part, down => down, strokes => strokes);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn an_online_change_keeps_a_sequence_with_its_wait_points_and_restarts_one_without() {
    // Specification 48 RU-73 with specification 44 §12.2. The running stamp waits for a part,
    // point 1, at 0 ms; the part comes before 10 ms, so it strokes from 10 ms, point 2. A change at
    // 20 ms to a stroke of 200 ms keeps the wait points and locals, so the state's member
    // `shape_2850a962` (the FNV-1a hash of `wait #1`, `wait #2`, computed independently) is carried
    // and so is the state: the stroke begun at 10 ms is still going at 200 ms and over at 210 ms,
    // when the stamp counts 1 and waits for the next part. It strokes again from 220 ms. A change at
    // 230 ms that adds a wait point before the stroke changes the shape to `shape_f0f8592b` (`wait
    // #1`, `wait #2`, `wait #3`): the report lists that member as new, the state is carried as it
    // was, point 2, and the next activation starts the sequence at its top, waiting for a part,
    // point 1; the outputs keep their values, the count 1 and the stamp down. At 240 ms the part,
    // still there, completes the wait and the added wait point begins, point 2. Catches a change
    // that resumes a sequence at a wait point whose number now means another, one that restarts a
    // sequence whose wait points stayed, and a restart the report does not name.
    let build = |name: &str, source: &str| {
        let root = custom_project(
            name,
            &[
                ("Cargo.toml", &manifest("")),
                ("src/lib.rs", &format!("{ROOT}{source}")),
            ],
            STAMPING,
        );
        built(&root)
    };
    let first = STAMP.replace("STROKE", "100");
    let longer = STAMP.replace("STROKE", "200");
    let added = first.replace(
        "            self.down = true;",
        "            s.next_cycle().await;\n            self.down = true;",
    );
    assert_ne!(added, first);
    let Some((first, device)) = build("rust-sequence-change-first", &first) else {
        return;
    };
    let Some((longer, _)) = build("rust-sequence-change-longer", &longer) else {
        return;
    };
    let Some((added, _)) = build("rust-sequence-change-added", &added) else {
        return;
    };
    let scan = |program: &mut trust_runtime::native::NativeProgram<
        '_,
        trust_runtime::native::HostCode,
    >,
                time: i64| {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    };
    let stamp =
        |program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>| {
            (
                program.value("CELL._SEQ.POINT"),
                program.value("DOWN"),
                program.value("STROKES"),
            )
        };
    let state = |point: u16, down: bool, strokes: i16| {
        (
            Some(Value::UInt(point)),
            Some(Value::Bool(down)),
            Some(Value::Int(strokes)),
        )
    };
    let names_the_shape = |events: &[CarriedValueEvent], shape: &str| {
        events.iter().any(|event| {
            matches!(event, CarriedValueEvent::New { name }
                if name.eq_ignore_ascii_case(&format!("Cell._seq.{shape}")))
        })
    };
    let mut running = load(&first.image, &device).expect("the image loads");
    running.cold_start(Duration::ZERO).expect("the cold start");
    scan(&mut running, 0);
    let (part, _) = running
        .image()
        .find_symbol("PART")
        .expect("the symbol of part");
    running
        .write_symbol(part, &Value::Bool(true))
        .expect("the part comes");
    scan(&mut running, 10);
    assert_eq!(stamp(&running), state(2, true, 0), "at 10 ms");

    let mut kept = load(&longer.image, &device).expect("the image loads");
    let report = kept
        .change_from(&mut running, Duration::from_millis(20))
        .expect("the change applies");
    assert!(
        !report.events.iter().any(|event| matches!(
            event,
            CarriedValueEvent::New { name } if name.to_ascii_lowercase().contains("shape_")
        )),
        "{:?}",
        report.events
    );
    scan(&mut kept, 200);
    assert_eq!(stamp(&kept), state(2, true, 0), "at 200 ms, 190 ms of 200");
    scan(&mut kept, 210);
    assert_eq!(stamp(&kept), state(1, false, 1), "at 210 ms, 200 ms");
    scan(&mut kept, 220);
    assert_eq!(stamp(&kept), state(2, true, 1), "at 220 ms, the next part");

    let mut restarted = load(&added.image, &device).expect("the image loads");
    let report = restarted
        .change_from(&mut kept, Duration::from_millis(230))
        .expect("the change applies");
    assert!(
        names_the_shape(&report.events, "shape_f0f8592b"),
        "{:?}",
        report.events
    );
    assert_eq!(
        stamp(&restarted),
        state(2, true, 1),
        "carried, before it runs"
    );
    scan(&mut restarted, 230);
    assert_eq!(
        stamp(&restarted),
        state(1, true, 1),
        "at 230 ms, at its top"
    );
    scan(&mut restarted, 240);
    assert_eq!(
        stamp(&restarted),
        state(2, true, 1),
        "at 240 ms, the added wait"
    );
}

/// A shuttle whose sequence drives to 10.0 and back, each drive a child sequence that takes its
/// target as a parameter.
const SHUTTLE: &str = "
/// Drives to 10.0 and back, each drive a child that takes its target.
#[trust_program(name = \"SHUTTLE\", sequence = \"run\")]
#[budget(\"60us\")]
pub struct Shuttle {
    #[input]
    pub arrived: bool,
    #[output]
    pub target: f64,
    #[output]
    pub trips: i16,
}

#[trust_sequence]
impl Shuttle {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            self.go(s, 10.0).await?;
            self.go(s, 0.0).await?;
        }
    }

    async fn go(&mut self, s: &mut Seq, goal: f64) -> Result<(), Fault> {
        self.target = goal;
        s.until(|me: &Self| me.arrived).await;
        self.trips = checked::add(s, self.trips, 1)?;
        Ok(())
    }
}
";

/// The configuration that runs SHUTTLE every 10 ms, its input and outputs connected to globals.
const SHUTTLING: &str = "CONFIGURATION Line
VAR_GLOBAL
    arrived : BOOL;
    target : LREAL;
    trips : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : SHUTTLE(arrived := arrived, target => target, trips => trips);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_configuration_runs_a_child_sequence_with_its_parameter() {
    // Specification 48 RU-71 and RU-72 in a project: the generated SHUTTLE_SEQ_GO holds the child's
    // parameter `goal`, and the program's code passes it. At 0 ms the shuttle awaits go(s, 10.0): the
    // child's `goal` is 10.0, it sets the target and waits, the shuttle at its point 1. Arrived before
    // 10 ms: the child counts the trip and returns, and the same activation awaits go(s, 0.0), `goal`
    // 0.0, point 2. At 20 ms, still arrived, the second trip, and go(s, 10.0) again, point 1.
    // Catches a parameter the declaration lays out apart from the macro's, and an argument that does
    // not reach the child.
    let root = custom_project(
        "rust-sequence-parameter",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{SHUTTLE}")),
        ],
        SHUTTLING,
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
    let shuttle =
        |program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>| {
            (
                program.value("CELL._SEQ.POINT"),
                program.value("CELL._SEQ.GO.GOAL"),
                program.value("TARGET"),
                program.value("TRIPS"),
            )
        };
    let state = |point: u16, goal: f64, target: f64, trips: i16| {
        (
            Some(Value::UInt(point)),
            Some(Value::LReal(goal)),
            Some(Value::LReal(target)),
            Some(Value::Int(trips)),
        )
    };
    scan(&mut program, 0);
    assert_eq!(shuttle(&program), state(1, 10.0, 10.0, 0), "at 0 ms");
    let (arrived, _) = program
        .image()
        .find_symbol("ARRIVED")
        .expect("the symbol of arrived");
    program
        .write_symbol(arrived, &Value::Bool(true))
        .expect("the shuttle arrives");
    scan(&mut program, 10);
    assert_eq!(shuttle(&program), state(2, 0.0, 0.0, 1), "at 10 ms");
    scan(&mut program, 20);
    assert_eq!(shuttle(&program), state(1, 10.0, 10.0, 2), "at 20 ms");
}

/// Two stations that work side by side, each a child sequence, as an SFC's simultaneous sequences.
const TWIN: &str = "
/// Counts a cycle once both stations have finished.
#[trust_program(name = \"TWIN\", sequence = \"run\")]
#[budget(\"60us\")]
pub struct Twin {
    #[input]
    pub left_done: bool,
    #[input]
    pub right_done: bool,
    #[output]
    pub cycles: i16,
}

#[trust_sequence]
impl Twin {
    async fn run(&mut self, s: &mut Seq) -> Result<(), Fault> {
        loop {
            s.all(self.left(s), self.right(s)).await;
            self.cycles = checked::add(s, self.cycles, 1)?;
            s.next_cycle().await;
        }
    }

    async fn left(&mut self, s: &mut Seq) -> Result<(), Fault> {
        s.until(|me: &Self| me.left_done).await;
        s.next_cycle().await;
        Ok(())
    }

    async fn right(&mut self, s: &mut Seq) -> Result<(), Fault> {
        s.until(|me: &Self| me.right_done).await;
        Ok(())
    }
}
";

/// The configuration that runs TWIN every 10 ms, its inputs and its output connected to globals.
const TWINNED: &str = "CONFIGURATION Line
VAR_GLOBAL
    left_done : BOOL;
    right_done : BOOL;
    cycles : INT;
END_VAR
RESOURCE Station ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Cell WITH Fast : TWIN(left_done := left_done, right_done := right_done, cycles => cycles);
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_configuration_runs_two_child_sequences_side_by_side() {
    // Specification 48 RU-72 in a project: an s.all whose members are the children `left` and
    // `right`. At 0 ms both start and wait, each at its point 1, the cell at the s.all, point 1.
    // The right station is done before 10 ms and returns: done 2. The left one is done before
    // 20 ms and waits a cycle more, its point 2. At 30 ms it returns: the cell counts 1 and waits a
    // cycle, point 2, done 0. Catches a generated state the children's members do not fit, a child
    // run after the other instead of beside it, and an s.all that does not wait for its slower
    // branch.
    let root = custom_project(
        "rust-sequence-twin",
        &[
            ("Cargo.toml", &manifest("")),
            ("src/lib.rs", &format!("{ROOT}{TWIN}")),
        ],
        TWINNED,
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
    let twin =
        |program: &trust_runtime::native::NativeProgram<'_, trust_runtime::native::HostCode>| {
            (
                program.value("CELL._SEQ.POINT"),
                program.value("CELL._SEQ.DONE"),
                program.value("CELL._SEQ.LEFT.POINT"),
                program.value("CELL._SEQ.RIGHT.POINT"),
                program.value("CYCLES"),
            )
        };
    let state = |point: u16, done: u16, left: u16, right: u16, cycles: i16| {
        (
            Some(Value::UInt(point)),
            Some(Value::Word(done)),
            Some(Value::UInt(left)),
            Some(Value::UInt(right)),
            Some(Value::Int(cycles)),
        )
    };
    scan(&mut program, 0);
    assert_eq!(twin(&program), state(1, 0, 1, 1, 0), "at 0 ms");
    write(&mut program, "RIGHT_DONE");
    scan(&mut program, 10);
    assert_eq!(twin(&program), state(1, 2, 1, 0, 0), "at 10 ms");
    write(&mut program, "LEFT_DONE");
    scan(&mut program, 20);
    assert_eq!(twin(&program), state(1, 2, 2, 0, 0), "at 20 ms");
    scan(&mut program, 30);
    assert_eq!(twin(&program), state(2, 0, 0, 0, 1), "at 30 ms");
}
