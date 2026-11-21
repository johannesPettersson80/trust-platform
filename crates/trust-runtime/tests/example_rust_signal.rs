//! The example `rust_signal` (specification 48 RU-88): the filtered level and the frame checks of
//! its README, which the program computes with the Rust function blocks `MEDIAN5` and `MODBUS_CRC`.
//! The test builds a copy of the example with the toolchain at `TRUST_TOOLCHAIN`, as **Start**
//! builds the project, and runs it scan by scan; the repository is never written.

mod rust_pous_support;

use rust_pous_support::{built_from, example_copy, project_sources};
use trust_runtime::io::IoAddress;
use trust_runtime::native::{load, HostCode, NativeProgram, NoHooks};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The task's interval, in milliseconds.
const INTERVAL_MS: i64 = 100;

/// Scans every 100 ms from `*time` up to `until` ms, then gives `names`' values.
fn run_to<const N: usize>(
    program: &mut NativeProgram<'_, HostCode>,
    time: &mut i64,
    until: i64,
    names: [&str; N],
) -> [Value; N] {
    while *time <= until {
        program
            .scan(Duration::from_millis(*time), &mut NoHooks)
            .expect("the scan runs");
        *time += INTERVAL_MS;
    }
    names.map(|name| program.value(name).expect(name))
}

/// The sensor's outputs: Raw, Level and Mean.
const SENSOR: [&str; 3] = [
    "MAININSTANCE.RAW",
    "MAININSTANCE.LEVEL",
    "MAININSTANCE.MEAN",
];

/// The serial line's outputs: FrameOk, Register and BadFrames.
const LINE: [&str; 3] = [
    "MAININSTANCE.FRAMEOK",
    "MAININSTANCE.REGISTER",
    "MAININSTANCE.BADFRAMES",
];

/// Raw, Level and Mean as REALs.
fn sensor(raw: f32, level: f32, mean: f32) -> [Value; 3] {
    [raw, level, mean].map(Value::Real)
}

/// FrameOk, Register and BadFrames.
fn line(ok: bool, register: i16, bad: i16) -> [Value; 3] {
    [Value::Bool(ok), Value::Int(register), Value::Int(bad)]
}

/// Puts `frame` at `%IW0`, the frame that comes in, as Live Values' Write does.
fn frame_in(program: &mut NativeProgram<'_, HostCode>, frame: u16) {
    let address = IoAddress::parse("%IW0").expect("the address");
    program
        .write_address(&address, Value::Word(frame))
        .expect("%IW0 is in the input image");
}

#[test]
fn the_median_holds_the_level_and_the_crc_refuses_a_changed_frame() {
    // The README's step by step, worked from the program: reading k comes at k x 0.5 s, and the
    // block's ring holds the last five (the first fills it). At 3.5 s the spike of 80.0 is in the
    // ring with 50.0, 50.3, 49.7 and 49.9: the median is 50.0 and the average 279.9 / 5 = 55.98. At
    // 4.0 s the ring holds 50.3, 49.7, 80.0, 50.1 and 50.0: median 50.1, average 280.1 / 5 = 56.02.
    // At 6.0 s the spike has left it: 50.2, 49.8, 50.0, 50.1 and 49.9, median and average 50.0. At
    // 7.0 s the dip to 20.0 comes in place of 49.9: median 50.0, average 220.1 / 5 = 44.02. Every average is the f32 of these decimals. Then the frame 01 03 02 00 2A
    // 39 9B, whose CRC is 0x9B39, is trusted with the register value 42; with 2A changed to 2B on the
    // line its CRC would be 0x5BF8, so it is refused, counted, and the register keeps 42; the frame
    // as sent again is trusted. Catches a filter that averages, a ring that loses a reading, a CRC
    // that trusts a changed frame, and a README whose values the program does not give.
    let root = example_copy("rust_signal");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut time = 0;
    for (until, expected) in [
        (3_500, sensor(80.0, 50.0, 55.98)),
        (4_000, sensor(50.1, 50.1, 56.02)),
        (6_000, sensor(50.0, 50.0, 50.0)),
        (7_000, sensor(20.0, 50.0, 44.02)),
    ] {
        assert_eq!(
            run_to(&mut program, &mut time, until, SENSOR),
            expected,
            "at {until} ms"
        );
    }
    assert_eq!(
        run_to(&mut program, &mut time, 7_000, LINE),
        line(false, 0, 0)
    );
    for (frame, expected) in [
        (1, line(true, 42, 0)),
        (2, line(false, 42, 1)),
        (1, line(true, 42, 1)),
    ] {
        frame_in(&mut program, frame);
        let next = time;
        assert_eq!(
            run_to(&mut program, &mut time, next, LINE),
            expected,
            "frame {frame}"
        );
    }
    let _ = std::fs::remove_dir_all(root);
}
