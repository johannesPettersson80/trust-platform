//! trust-sim against the trust-runtime this build made (specification 48 RU-58): a machine test of a
//! project's program writes, forces and reads variables by their paths, runs cycles and logical
//! time, and sees a fault.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::{Path, PathBuf};
use std::time::Duration;

/// A resource that runs a cycle every 10 ms and halts on a fault.
const RUNTIME_TOML: &str = "[bundle]
version = 1

[resource]
name = \"Tank\"
cycle_interval_ms = 10

[runtime.control]
mode = \"production\"
debug_enabled = false

[runtime.log]
level = \"info\"

[runtime.retain]
mode = \"none\"
save_interval_ms = 1000

[runtime.watchdog]
enabled = false
timeout_ms = 1000
action = \"halt\"

[runtime.fault]
policy = \"halt\"
";

const IO_TOML: &str = "[io]\ndriver = \"simulated\"\nparams = {}\n";

/// A level alarm above 90, a cycle count, and a division that faults on a zero divisor.
const MAIN_ST: &str = "
PROGRAM Main
VAR
    level : REAL := 0.0;
    alarm : BOOL;
    cycles : DINT := 0;
    divisor : DINT := 1;
    quotient : DINT;
END_VAR
cycles := cycles + 1;
alarm := level > 90.0;
quotient := 100 / divisor;
END_PROGRAM
";

/// A sequence that an operator starts with a set point and resets, through the methods of its
/// interface, and a program that copies its state.
const SEQUENCE_ST: &str = "
INTERFACE ISequence
METHOD Start : BOOL
VAR_INPUT Setpoint : REAL; END_VAR
END_METHOD
METHOD Reset
END_METHOD
END_INTERFACE

FUNCTION_BLOCK Sequence IMPLEMENTS ISequence
VAR_OUTPUT running : BOOL; target : REAL; END_VAR
METHOD PUBLIC Start : BOOL
VAR_INPUT Setpoint : REAL; END_VAR
    running := TRUE;
    target := Setpoint;
    Start := TRUE;
END_METHOD
METHOD PUBLIC Reset
    running := FALSE;
END_METHOD
END_FUNCTION_BLOCK

PROGRAM Main
VAR
    Heating : Sequence;
    running : BOOL;
    target : REAL;
END_VAR
Heating();
running := Heating.running;
target := Heating.target;
END_PROGRAM
";

/// A duration and its double, a date, and the date and time of the cycle (`CURRENT_DT()`).
const TIMES_ST: &str = "
PROGRAM Main
VAR
    hold : TIME := T#2s;
    doubled : TIME;
    day : DATE := D#2026-11-23;
    stamp : DT;
END_VAR
doubled := hold + hold;
stamp := CURRENT_DT();
END_PROGRAM
";

fn project() -> PathBuf {
    project_with(MAIN_ST)
}

/// A project of the resource above whose program is `main_st`.
fn project_with(main_st: &str) -> PathBuf {
    let project = trust_test_support::unique_temp_path("trust-sim-machine");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), IO_TOML).expect("write io.toml");
    std::fs::write(project.join("src/main.st"), main_st).expect("write main.st");
    project
}

/// The simulator of `project`, run by the trust-runtime this build made.
fn simulator(project: &Path) -> trust_sim::Result<trust_sim::Simulator> {
    let runtime = std::env::var_os("CARGO_BIN_EXE_trust-runtime")
        .expect("Cargo provides trust-runtime to its integration tests");
    trust_sim::Project::open_with_runtime(project, runtime)?.simulator()
}

#[test]
fn a_machine_test_writes_forces_reads_and_sees_a_fault() -> trust_sim::Result<()> {
    // Worked by hand on a 10 ms resource. A level of 95 written before the first cycle raises the
    // alarm in the cycles at 0, 10 and 20 ms, so the time is 20 ms and 3 cycles counted. A level
    // forced to 40 holds through the 50 ms that follow, the cycles at 30, 40, 50, 60 and 70 ms, so
    // the alarm falls and the time ends at 70 ms. A zero divisor faults the next cycle, at 80 ms,
    // which ends a run of 5 there with the error `division by zero in Main at src/main.st:12`, the
    // division's line in the source, which starts with an empty line (specification 48 RU-58). A
    // path that names no variable is refused. Catches a write that misses the next cycle, a force
    // the program overwrites, a duration that runs past its end, a fault that goes unseen or names
    // no place, and an unknown path read as a value.
    let project = project();
    let mut plant = simulator(&project)?;

    plant.write("Main.level", 95.0)?;
    plant.run_cycles(3)?;
    assert!(plant.read::<bool>("Main.alarm")?);
    assert_eq!(plant.read::<i32>("Main.cycles")?, 3);
    assert_eq!(plant.now(), Duration::from_millis(20));

    plant.force("Main.level", 40.0)?;
    plant.run_for(Duration::from_millis(50))?;
    assert!(!plant.read::<bool>("Main.alarm")?);
    assert_eq!(plant.read::<f64>("Main.level")?, 40.0);
    assert_eq!(plant.read::<i32>("Main.cycles")?, 8);
    assert_eq!(plant.now(), Duration::from_millis(70));
    plant.release("Main.level")?;

    plant.write("Main.divisor", 0)?;
    let faulted = plant.run_cycles(5);
    assert!(
        matches!(&faulted, Err(trust_sim::Error::Fault { time, fault })
            if *time == Duration::from_millis(80)
                && fault == "division by zero in Main at src/main.st:12"),
        "{faulted:?}"
    );
    assert!(matches!(
        plant.read::<bool>("Main.missing"),
        Err(trust_sim::Error::Refused { .. })
    ));

    drop(plant);
    let _ = std::fs::remove_dir_all(project);
    Ok(())
}

#[test]
fn a_machine_test_presses_a_sequences_start_and_reset() -> trust_sim::Result<()> {
    // Worked by hand on the 10 ms resource above: the cycle at 0 ms runs the sequence that nothing
    // started, so `Main.running` is FALSE. `Start` with a set point of 21.5 answers TRUE, and the
    // cycle at 10 ms copies running TRUE and the target 21.5 into the program. `Reset`, a method
    // without a result, answers `()`, and the cycle at 20 ms copies running FALSE. A result asked
    // for as a number is `Error::Kind`, and a method the interface does not declare is
    // `Error::Refused`. Catches a call that never reaches the program, an input not passed, a
    // result read as another kind, and `()` refused for a method without a result.
    let project = project_with(SEQUENCE_ST);
    let mut plant = simulator(&project)?;

    plant.run_cycles(1)?;
    assert!(!plant.read::<bool>("Main.running")?);

    let started: bool = plant.call(
        "Main.Heating",
        "ISequence",
        "Start",
        trust_sim::Inputs::new().with("Setpoint", 21.5),
    )?;
    assert!(started);
    plant.run_cycles(1)?;
    assert!(plant.read::<bool>("Main.running")?);
    assert_eq!(plant.read::<f64>("Main.target")?, 21.5);

    plant.call::<()>(
        "Main.Heating",
        "ISequence",
        "Reset",
        trust_sim::Inputs::new(),
    )?;
    plant.run_cycles(1)?;
    assert!(!plant.read::<bool>("Main.running")?);
    assert_eq!(plant.now(), Duration::from_millis(20));

    let number = plant.call::<i32>(
        "Main.Heating",
        "ISequence",
        "Start",
        trust_sim::Inputs::new().with("Setpoint", 5.0),
    );
    assert!(
        matches!(number, Err(trust_sim::Error::Kind { .. })),
        "{number:?}"
    );
    let missing = plant.call::<()>(
        "Main.Heating",
        "ISequence",
        "Stop",
        trust_sim::Inputs::new(),
    );
    assert!(
        matches!(missing, Err(trust_sim::Error::Refused { .. })),
        "{missing:?}"
    );

    drop(plant);
    let _ = std::fs::remove_dir_all(project);
    Ok(())
}

#[test]
fn a_machine_test_reads_and_writes_times_and_dates() -> trust_sim::Result<()> {
    // Specification 48 RU-58: a value is a Rust value of the variable's type, trust_plc's times and
    // dates among them. Worked by hand on the 10 ms resource above: the cycle at 0 ms keeps the
    // initial T#2s and D#2026-11-23, day 20_780 after 1970-01-01, and doubles the duration to 4 s. A
    // written 3 s doubles to 6 s at the cycle at 10 ms, whose CURRENT_DT() is the logical time from
    // 1970, DT#1970-01-01-00:00:00.010; a written D#2000-03-01, day 11_017, reads back. Catches a
    // duration or a date written in the wrong unit, and a date off by a leap day.
    use trust_sim::{Date, Dt, Time};
    let project = project_with(TIMES_ST);
    let mut plant = simulator(&project)?;

    plant.run_cycles(1)?;
    assert_eq!(
        plant.read::<Time>("Main.hold")?,
        Time::from_nanos(2_000_000_000)
    );
    assert_eq!(
        plant.read::<Time>("Main.doubled")?,
        Time::from_nanos(4_000_000_000)
    );
    assert_eq!(
        plant.read::<Date>("Main.day")?,
        Date::from_millis(20_780 * 86_400_000)
    );

    plant.write("Main.hold", Time::from_nanos(3_000_000_000))?;
    plant.write("Main.day", Date::from_millis(11_017 * 86_400_000))?;
    plant.run_cycles(1)?;
    assert_eq!(
        plant.read::<Time>("Main.doubled")?,
        Time::from_nanos(6_000_000_000)
    );
    assert_eq!(
        plant.read::<Date>("Main.day")?,
        Date::from_millis(11_017 * 86_400_000)
    );
    assert_eq!(plant.read::<Dt>("Main.stamp")?, Dt::from_millis(10));

    drop(plant);
    let _ = std::fs::remove_dir_all(project);
    Ok(())
}
