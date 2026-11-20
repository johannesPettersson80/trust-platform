//! The Rust starts of a new project build and run as they are created (specification 25 §3.2,
//! specification 48 RU-51, RU-76): `trust-runtime new --rust` and `--rust-first` write every file,
//! the generated Structured Text among them, so that the project builds with the toolchain alone.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. A toolchain without
//! cargo and the SDK's sources refuses the build with that reason.

mod rust_pous_support;

use rust_pous_support::{built_from, project_sources};
use trust_runtime::native::{load, NoHooks};
use trust_runtime::new_project::{create_project_as, ProjectStart};
use trust_runtime::stdlib::conversions::ConversionProfile;
use trust_runtime::value::{Duration, Value};

#[test]
fn a_new_structured_text_project_with_rust_smooths_its_input_with_the_crates_block() {
    // Specification 48 RU-51, worked by hand: `src/Main.st` calls FILTER_EMA with alpha 0.1 on the
    // 10 ms task, which is due at its start (specification 10 §4.3). At 0 ms the block takes Raw = 0
    // as it is; the test then writes Raw = 100, and the calls at 10 and 20 ms move Smoothed a tenth
    // of the gap: 0 + 0.1 × 100 = 10, then 10 + 0.1 × 90 = 19, both exact in binary. Catches a
    // start whose project does not build as created (a missing or stale declaration, a crate the
    // build refuses) or whose program does not call the crate's block.
    let parent = trust_test_support::unique_temp_path("new-rust-project-filter");
    let root = create_project_as(
        &parent,
        "Filter Demo",
        ProjectStart::StructuredTextWithRust,
        false,
    )
    .expect("the project");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    // The task is due at its start (specification 10 §4.3): the scan at 0 ms is its activation.
    program
        .scan(Duration::ZERO, &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(
        program.value("MAININSTANCE.SMOOTHED"),
        Some(Value::LReal(0.0))
    );
    let (raw, _) = program
        .image()
        .find_symbol("MAININSTANCE.RAW")
        .expect("Raw's symbol");
    program
        .write_symbol(raw, &Value::LReal(100.0))
        .expect("Raw written");
    for (time, smoothed) in [(10, 10.0), (20, 19.0)] {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
        assert_eq!(
            program.value("MAININSTANCE.SMOOTHED"),
            Some(Value::LReal(smoothed)),
            "at {time} ms"
        );
    }
    let _ = std::fs::remove_dir_all(parent);
}

#[test]
fn a_new_rust_first_project_switches_its_lamp_every_fifty_runs() {
    // Specification 48 RU-51 and §14, worked by hand: `trust.toml` runs MAIN every 10 ms from its
    // start (specification 10 §4.3), so it has run 49 times at 480 ms, its lamp still off, and its
    // 50th run at 490 ms switches it on. Catches a Rust-first start whose configuration does not run its program, or
    // whose project does not build as created.
    let parent = trust_test_support::unique_temp_path("new-rust-project-lamp");
    let root =
        create_project_as(&parent, "cell", ProjectStart::RustFirst, false).expect("the project");
    let Some((built, device)) = built_from(&root, &project_sources(&root), ConversionProfile::Iec)
    else {
        return;
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");
    for time in (0..=480).step_by(10) {
        program
            .scan(Duration::from_millis(time), &mut NoHooks)
            .expect("the scan runs");
    }
    assert_eq!(program.value("MAININSTANCE.LAMP"), Some(Value::Bool(false)));
    program
        .scan(Duration::from_millis(490), &mut NoHooks)
        .expect("the scan runs");
    assert_eq!(program.value("MAININSTANCE.LAMP"), Some(Value::Bool(true)));
    let _ = std::fs::remove_dir_all(parent);
}
