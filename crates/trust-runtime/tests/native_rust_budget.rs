//! A debug image times each call of a Rust POU on the kernel's clock and keeps, in the POU's
//! record, its longest call and its calls over its budget for the image's target; a release image
//! measures nothing (specification 48 RU-49; specification 44 §5.13, §8.2).
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources refuses the build with that reason, and the test ends there.

mod rust_pous_support;

use std::path::{Path, PathBuf};

use rust_pous_support::{built, custom_project, manifest, sources};
use trust_runtime::native::{build_project, load, triple, NoHooks, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::Duration;
use trust_runtime_core::native::image::{Image, SectionId};

/// The crate: SPIN, which adds up far more numbers than its budget of 1 µs allows; Quick, which
/// takes far less than its budget of 100 ms; and the block Counter with its method `reset`, which
/// no Structured Text calls.
const CRATE: &str = r#"#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// The sum of the numbers below `n`, added one at a time.
#[trust_function(name = "SPIN")]
#[budget("1us")]
pub fn spin(_cx: &mut Cx, n: i32) -> Result<i32, Fault> {
    let mut sum = 0i32;
    for number in 0..n {
        sum = sum.wrapping_add(core::hint::black_box(number));
    }
    Ok(sum)
}

/// One more than `a`.
#[trust_function(name = "Quick")]
#[budget("100ms")]
pub fn quick(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(a.wrapping_add(1))
}

/// Counts nothing; no Structured Text calls it.
#[trust_fb(name = "Counter")]
#[budget("2us")]
pub struct Counter {
    #[output]
    pub count: i16,
}

impl Body for Counter {
    fn body(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        Ok(())
    }
}

impl Counter {
    /// Starts the count again.
    #[trust_method]
    #[budget("3us")]
    pub fn reset(&mut self, _cx: &mut Cx) -> Result<(), Fault> {
        self.count = 0;
        Ok(())
    }
}
"#;

/// The program, which calls SPIN and Quick once per activation of its task every 100 ms.
const PROGRAM: &str = "PROGRAM Line
VAR
    total : DINT;
    next : DINT;
END_VAR
total := SPIN(200000);
next := Quick(next);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#100ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// The project, whose `trust.toml` lowers Quick's budget to 50 ms for this computer's target.
fn project(name: &str) -> PathBuf {
    let root = custom_project(
        name,
        &[("Cargo.toml", manifest("").as_str()), ("src/lib.rs", CRATE)],
        PROGRAM,
    );
    let target = triple(Target::this_cpu().expect("a computer target"));
    let file = root.join("trust.toml");
    let mut text = std::fs::read_to_string(&file).unwrap_or_default();
    text.push_str(&format!(
        "\n[timing.budgets]\nQuick = {{ \"{target}\" = \"50ms\" }}\n"
    ));
    std::fs::write(&file, text).expect("trust.toml");
    root
}

/// Four scans at 0, 100, 200 and 300 ms: three activations of the task, which is not due at 0
/// (specification 10 §4.3). Each Rust POU's record after them, as its budget, its longest call and
/// its calls over the budget, in the order of the Rust POU table.
fn three_activations(image: &[u8], device: &trust_runtime::native::Device) -> Vec<(i64, i64, u64)> {
    let mut program = load(image, device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in [0, 100, 200, 300] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    let names: Vec<_> = (0..u32::try_from(program.rust_times().len()).expect("a count"))
        .map(|pou| program.rust_pou_name(pou).expect("a name").to_string())
        .collect();
    if !names.is_empty() {
        assert_eq!(names, ["Counter", "Counter.reset", "Quick", "SPIN"]);
    }
    program
        .rust_times()
        .iter()
        .map(|time| (time.budget_ns, time.longest_ns, time.over))
        .collect()
}

#[test]
fn a_debug_image_times_each_rust_pou_call_against_its_budget() {
    // RU-49, three activations: SPIN adds up 200,000 numbers, far longer than its 1 µs, so all
    // three calls are over its budget and its longest is above it; Quick's three calls stay within
    // its budget, which trust.toml lowers from 100 ms to 50 ms for this target; Counter and its
    // method, which no Structured Text calls, keep records of no call. The records follow the names
    // in capitals: COUNTER, COUNTER.RESET, QUICK, SPIN. Catches a call noted in another POU's
    // record, a budget not lowered for the target, a call within its budget counted over it and a
    // table without the POUs the program does not call.
    let root = project("rust-budget-debug");
    let Some((built, device)) = built(&root) else {
        return;
    };
    let times = three_activations(&built.image, &device);
    let budgets: Vec<_> = times.iter().map(|(budget, _, _)| *budget).collect();
    assert_eq!(budgets, [2_000, 3_000, 50_000_000, 1_000]);
    assert_eq!(times[0], (2_000, 0, 0), "Counter is never called");
    assert_eq!(times[1], (3_000, 0, 0), "Counter.reset is never called");
    let (_, quick_longest, quick_over) = times[2];
    assert_eq!(quick_over, 0, "Quick: {:?}", times[2]);
    assert!(quick_longest < 50_000_000, "Quick: {:?}", times[2]);
    let (_, spin_longest, spin_over) = times[3];
    assert_eq!(spin_over, 3, "SPIN: {:?}", times[3]);
    assert!(spin_longest > 1_000, "SPIN: {:?}", times[3]);
}

#[test]
fn a_release_image_measures_no_rust_pou_call() {
    // RU-49: a release image has no Rust POU table and its kernel gives it no records, so its
    // calls are not timed. Catches a release image that reads the clock or carries the table.
    let root = project("rust-budget-release");
    let Some((built, device)) = release(&root) else {
        return;
    };
    let image = Image::parse(&built.image).expect("the image parses");
    assert_eq!(image.section(SectionId::RustPous), None);
    assert_eq!(three_activations(&built.image, &device), []);
}

/// The project built for this computer as a release image; `None` with a toolchain that has no
/// cargo and SDK sources.
fn release(root: &Path) -> Option<(trust_runtime::native::Build, trust_runtime::native::Device)> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return None;
    }
    let target = Target::this_cpu().expect("a computer target");
    let built = build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target,
            hooks: false,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .unwrap_or_else(|error| panic!("the project builds: {error}"));
    let device = trust_runtime::native::Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    Some((built, device))
}
