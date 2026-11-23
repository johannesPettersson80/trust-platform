//! trust-sim against the trust-runtime this build made (specification 48 RU-58): a machine test of a
//! project's program writes, forces and reads variables by their paths, runs cycles and logical
//! time, and sees a fault.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::PathBuf;
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

fn project() -> PathBuf {
    let project = trust_test_support::unique_temp_path("trust-sim-machine");
    std::fs::create_dir_all(project.join("src")).expect("create the project");
    std::fs::write(project.join("runtime.toml"), RUNTIME_TOML).expect("write runtime.toml");
    std::fs::write(project.join("io.toml"), IO_TOML).expect("write io.toml");
    std::fs::write(project.join("src/main.st"), MAIN_ST).expect("write main.st");
    project
}

#[test]
fn a_machine_test_writes_forces_reads_and_sees_a_fault() -> trust_sim::Result<()> {
    // Worked by hand on a 10 ms resource. A level of 95 written before the first cycle raises the
    // alarm in the cycles at 0, 10 and 20 ms, so the time is 20 ms and 3 cycles counted. A level
    // forced to 40 holds through the 50 ms that follow, the cycles at 30, 40, 50, 60 and 70 ms, so
    // the alarm falls and the time ends at 70 ms. A zero divisor faults the next cycle, at 80 ms,
    // which ends a run of 5 there. A path that names no variable is refused. Catches a write that
    // misses the next cycle, a force the program overwrites, a duration that runs past its end, a
    // fault that goes unseen, and an unknown path read as a value.
    let project = project();
    let runtime = std::env::var_os("CARGO_BIN_EXE_trust-runtime")
        .expect("Cargo provides trust-runtime to its integration tests");
    let mut plant = trust_sim::Project::open_with_runtime(&project, runtime)?.simulator()?;

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
        matches!(faulted, Err(trust_sim::Error::Fault { time, .. }) if time == Duration::from_millis(80)),
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
