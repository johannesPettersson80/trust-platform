//! The example `rust_barcode` (specification 48 RU-88): the packing line's verdict on each of its
//! four labels, as its README gives them, which the program computes with the Rust function block
//! `GS1_LABEL`. The test builds a copy of the example with the toolchain at `TRUST_TOOLCHAIN`, as
//! **Start** builds the project, and runs it scan by scan; the repository is never written.

mod rust_pous_support;

use rust_pous_support::{built_from, example_copy, project_sources};
use trust_runtime::io::IoAddress;
use trust_runtime::native::{load, HostCode, NativeProgram, NoHooks};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

/// The task's interval, in milliseconds.
const INTERVAL_MS: i64 = 100;

/// The verdict's outputs: Accepted, Expired, Unreadable, WrongProduct and Problem.
const VERDICT: [&str; 5] = [
    "MAININSTANCE.ACCEPTED",
    "MAININSTANCE.EXPIRED",
    "MAININSTANCE.UNREADABLE",
    "MAININSTANCE.WRONGPRODUCT",
    "MAININSTANCE.PROBLEM",
];

/// Puts `label` at `%IW0`, the label the simulated scanner reads, as Live Values' Write does, and
/// runs the task once; gives the verdict.
fn read_label(program: &mut NativeProgram<'_, HostCode>, time: &mut i64, label: u16) -> [Value; 5] {
    let address = IoAddress::parse("%IW0").expect("the address");
    program
        .write_address(&address, Value::Word(label))
        .expect("%IW0 is in the input image");
    program
        .scan(Duration::from_millis(*time), &mut NoHooks)
        .expect("the scan runs");
    *time += INTERVAL_MS;
    VERDICT.map(|name| program.value(name).expect(name))
}

/// A verdict: Accepted, Expired, Unreadable, WrongProduct and Problem.
fn verdict(flags: [bool; 4], problem: i16) -> [Value; 5] {
    let [accepted, expired, unreadable, wrong] = flags.map(Value::Bool);
    [accepted, expired, unreadable, wrong, Value::Int(problem)]
}

#[test]
fn each_label_gets_the_verdict_its_readme_gives() {
    // The README's step by step, worked from the labels and GS1's rules, on the day 2026-11-20:
    // 1. GTIN 09501101530003, whose check digit is 3 (its first 13 digits, weighted 3 and 1 from the
    //    right, add up to 47, and 50 - 47 = 3), batch ABC123, expiry 2026-12-31: accepted.
    // 2. The same GTIN with the expiry 250600: the day 00 is June's last, 2025-06-30, before the day
    //    the line runs: expired.
    // 3. GTIN 09501101530004: read, problem 0, but its last digit is not the check digit 3:
    //    unreadable.
    // 4. GTIN 05012345678900, whose check digit 0 is right (the sum is 90): another product than
    //    09501101530003: wrong product.
    // 0 names no label: no verdict. Catches an example that does not build as committed, a reader
    // that keeps a day of 00, a check digit that is not checked, and a README whose verdicts the
    // program does not give.
    let root = example_copy("rust_barcode");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    let mut time = 0;
    let none = verdict([false; 4], 0);
    assert_eq!(read_label(&mut program, &mut time, 0), none);

    assert_eq!(
        read_label(&mut program, &mut time, 1),
        verdict([true, false, false, false], 0)
    );
    assert_eq!(
        ["MAININSTANCE.READ.GTIN", "MAININSTANCE.READ.BATCH"]
            .map(|name| program.value(name).expect(name)),
        [
            Value::String("09501101530003".into()),
            Value::String("ABC123".into())
        ]
    );
    assert_eq!(
        read_label(&mut program, &mut time, 2),
        verdict([false, true, false, false], 0)
    );
    assert_eq!(
        read_label(&mut program, &mut time, 3),
        verdict([false, false, true, false], 0)
    );
    assert_eq!(
        read_label(&mut program, &mut time, 4),
        verdict([false, false, false, true], 0)
    );
    assert_eq!(read_label(&mut program, &mut time, 0), none);
    let _ = std::fs::remove_dir_all(root);
}
