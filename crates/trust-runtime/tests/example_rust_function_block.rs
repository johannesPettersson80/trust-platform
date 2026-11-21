//! The example `rust_function_block` (specification 48 RU-88): its README's values are the ones its
//! program computes. The test builds a copy of the example with the toolchain at `TRUST_TOOLCHAIN`,
//! as **Start** builds the project, and runs it scan by scan; the repository is never written. A
//! toolchain without cargo and the SDK's sources refuses the build with that reason.

mod rust_pous_support;

use rust_pous_support::{built_from, example_copy, project_sources};
use trust_runtime::io::IoAddress;
use trust_runtime::native::{load, HostCode, NativeProgram, NoHooks};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The README's task interval, in milliseconds.
const INTERVAL_MS: i64 = 100;

/// Writes `setpoint` into `Setpoint`, `%IL0`, in the input image that the next scan samples, as Live
/// Values' Write of the simulated input does. The image is untyped: the long word holds the
/// `LREAL`'s bits.
fn write_setpoint(program: &mut NativeProgram<'_, HostCode>, setpoint: f64) {
    let address = IoAddress::parse("%IL0").expect("the address");
    program
        .write_address(&address, Value::LWord(setpoint.to_bits()))
        .expect("%IL0 is in the input image");
}

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

/// Runs to each step's time, checking Speed and AtSpeed there.
fn expect_ramp(
    program: &mut NativeProgram<'_, HostCode>,
    time: &mut i64,
    steps: &[(i64, f64, bool)],
) {
    for &(until, speed, at_speed) in steps {
        assert_eq!(
            run_to(program, time, until),
            (Value::LReal(speed), Value::Bool(at_speed)),
            "at {until} ms"
        );
    }
}

#[test]
fn the_examples_ramp_moves_the_speed_as_its_readme_says() {
    // Specification 48 RU-88, the README's step by step, worked by hand: the task runs at its start
    // and then every 100 ms (specification 10 §4.3), and RAMP moves Speed by rate x interval =
    // 10 x 0.1 s = 1.0 a run. The setpoint input starts at 0.0, so Speed is 0.0 and AtSpeed TRUE.
    // With 50.0 written before the run at 1 s, Speed is 1.0 then, 49.0 with AtSpeed FALSE at 5.8 s,
    // the 49th run with the new setpoint, and 50.0 with AtSpeed TRUE at 5.9 s, the 50th. With 20.0
    // written after that run it falls by 1.0 a run: 21.0 with AtSpeed FALSE at 8.8 s, the 29th run
    // after the write, and 20.0 with AtSpeed TRUE at 8.9 s, the 30th. Every value is exact in binary.
    // Catches an example that does not build as committed (stale declarations, a crate the build
    // refuses), a setpoint the program does not take from its input, and a README whose values the
    // program does not compute.
    let root = example_copy("rust_function_block");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut time = 0;
    expect_ramp(&mut program, &mut time, &[(0, 0.0, true), (900, 0.0, true)]);
    write_setpoint(&mut program, 50.0);
    expect_ramp(
        &mut program,
        &mut time,
        &[
            (1_000, 1.0, false),
            (5_800, 49.0, false),
            (5_900, 50.0, true),
        ],
    );
    write_setpoint(&mut program, 20.0);
    expect_ramp(
        &mut program,
        &mut time,
        &[(8_800, 21.0, false), (8_900, 20.0, true)],
    );
    let _ = std::fs::remove_dir_all(root);
}
