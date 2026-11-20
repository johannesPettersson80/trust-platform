//! The example `rust_function_block` (specification 48 RU-88): its README's values are the ones its
//! program computes. The test builds a copy of the example with the toolchain at `TRUST_TOOLCHAIN`,
//! as **Start** builds the project, and runs it scan by scan; the repository is never written. A
//! toolchain without cargo and the SDK's sources refuses the build with that reason.

mod rust_pous_support;

use rust_pous_support::{built_from, example_copy, project_sources};
use trust_runtime::native::{load, HostCode, NativeProgram, NoHooks};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The README's task interval, in milliseconds.
const INTERVAL_MS: i64 = 100;

/// Scans `program` every 100 ms from `*time` up to `until` ms, then gives Speed and AtSpeed.
fn run_to(program: &mut NativeProgram<'_, HostCode>, time: &mut i64, until: i64) -> (Value, Value) {
    while *time <= until {
        program
            .scan(Duration::from_millis(*time), &mut NoHooks)
            .expect("the scan runs");
        *time += INTERVAL_MS;
    }
    (
        program.value("MAININSTANCE.SPEED").expect("Speed"),
        program.value("MAININSTANCE.ATSPEED").expect("AtSpeed"),
    )
}

#[test]
fn the_examples_ramp_moves_the_speed_as_its_readme_says() {
    // Specification 48 RU-88, the README's step by step, worked by hand: RAMP moves Speed by rate x
    // interval = 10 x 0.1 s = 1.0 at each run of the 100 ms task, which runs at its start and then
    // every 100 ms (specification 10 §4.3). So Speed is 11.0 at 1 s (runs at 0, 0.1, ..., 1 s), 49.0
    // with AtSpeed FALSE at 4.8 s, and 50.0 with AtSpeed TRUE at 4.9 s, the 50th run. With Setpoint 20.0,
    // written after that run, it falls by 1.0 a run: 21.0 with AtSpeed FALSE at 7.8 s, the 29th run
    // after the write, and 20.0 with AtSpeed TRUE at 7.9 s, the 30th. Every value is exact in binary. Catches an example
    // that does not build as committed (stale declarations, a crate the build refuses) and a README
    // whose values the program does not compute.
    let root = example_copy("rust_function_block");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut time = 0;
    for (until, speed, at_speed) in [
        (1_000, 11.0, false),
        (4_800, 49.0, false),
        (4_900, 50.0, true),
    ] {
        assert_eq!(
            run_to(&mut program, &mut time, until),
            (Value::LReal(speed), Value::Bool(at_speed)),
            "at {until} ms"
        );
    }
    let (setpoint, _) = program
        .image()
        .find_symbol("MAININSTANCE.SETPOINT")
        .expect("Setpoint's symbol");
    program
        .write_symbol(setpoint, &Value::LReal(20.0))
        .expect("Setpoint written");
    for (until, speed, at_speed) in [(7_800, 21.0, false), (7_900, 20.0, true)] {
        assert_eq!(
            run_to(&mut program, &mut time, until),
            (Value::LReal(speed), Value::Bool(at_speed)),
            "at {until} ms"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}
