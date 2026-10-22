//! A program without read-only data compiles into a module whose code starts at address 0, for the
//! C6 and for this computer, and runs (specification 44 §4.5, §5.3).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::SourceFile;
use trust_runtime::native::{build, load, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};

/// A counter in a 10 ms task: no constant table, no text, nothing a module keeps read-only.
const COUNTER: &str = "
PROGRAM Counter
VAR
    Count : UDINT;
END_VAR
Count := Count + 1;
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Counter;
END_RESOURCE
END_CONFIGURATION
";

#[test]
fn a_program_without_read_only_data_builds_for_the_c6_and_runs_here() {
    // A module whose linker placed its dynamic symbol tables before its code was refused with "the
    // module's code does not start at address 0" for the C6 (22 October 2026, the fault probe); the
    // reference workload has read-only data and did not show it. Catches the tables placed before
    // the code again, for either target.
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let sources = [SourceFile::new(COUNTER)];
    let host = Target::this_cpu().expect("a computer target");
    for target in [Target::Rv32imac, host] {
        let work = std::env::temp_dir().join(format!(
            "trust-native-small-{}-{target:?}",
            std::process::id()
        ));
        let built = build(
            &sources,
            &Options {
                target,
                hooks: true,
            },
            &toolchain,
            &work,
        )
        .unwrap_or_else(|error| panic!("the counter builds for {target:?}: {error}"));
        let _ = std::fs::remove_dir_all(&work);
        assert_eq!(built.figure.rodata, 0, "the counter has no read-only data");
        if target != host {
            continue;
        }
        let device = Device {
            target,
            program_memory: 1 << 20,
            task_stack: 1 << 20,
            max_statements: 1_000,
        };
        let mut program = load(&built.image, &device).expect("the image loads");
        program.cold_start(Duration::ZERO).expect("the cold start");
        // The task is due at 10, 20 and 30 ms, not at its start (specification 10 §4.3).
        for time in [0, 10, 20, 30] {
            program
                .scan(Duration::from_millis(time), &mut NoHooks)
                .expect("the scan runs");
        }
        assert_eq!(program.value("MAIN.COUNT"), Some(Value::UDInt(3)));
    }
}
