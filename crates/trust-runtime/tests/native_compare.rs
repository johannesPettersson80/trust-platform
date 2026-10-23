//! A project compiled into a native module and run by the kernel computes what today's runtime
//! computes, variable for variable, scan after scan (specification 44 §9, §14.2): both run the
//! project's configuration at its cycle interval in lockstep, and after every scan each variable
//! the image's symbol table names a value of is compared with the runtime's.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;

#[path = "support/native_lockstep.rs"]
#[allow(dead_code)]
mod native_lockstep;

use native_lockstep::compare;

/// The PLCopen motion single-axis demonstration, with its library of motion function blocks that
/// take the axis as a `VAR_IN_OUT`, runs alike for five seconds of 10 ms scans.
#[test]
fn the_motion_demonstration_runs_as_the_runtime_runs_it() {
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/plcopen_motion_single_axis_demo");
    match compare(&project, 10, 500) {
        Ok(matched) => println!("{matched} values matched"),
        Err(divergence) => panic!("{divergence}"),
    }
}

/// The PLCopen motion feeder, an object-oriented program: an axis class used through its
/// interface, commands returned as interfaces and read through their properties, classes that
/// extend others and a method with an output, runs alike for five seconds of 10 ms scans.
#[test]
fn the_object_oriented_feeder_runs_as_the_runtime_runs_it() {
    let project =
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/plcopen_motion_oop_feeder_axis");
    match compare(&project, 10, 500) {
        Ok(matched) => println!("{matched} values matched"),
        Err(divergence) => panic!("{divergence}"),
    }
}

/// The project at `TRUST_COMPARE_PROJECT`, `TRUST_COMPARE_SCANS` scans (100) of
/// `TRUST_COMPARE_CYCLE_MS` (10): the comparison of the corpus, project by project.
#[test]
#[ignore = "runs the project TRUST_COMPARE_PROJECT names"]
fn the_named_project_runs_as_the_runtime_runs_it() {
    let project = std::env::var("TRUST_COMPARE_PROJECT").expect("TRUST_COMPARE_PROJECT");
    let number = |name: &str, default: i64| {
        std::env::var(name)
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(default)
    };
    let scans = number("TRUST_COMPARE_SCANS", 100);
    let cycle = number("TRUST_COMPARE_CYCLE_MS", 10);
    match compare(Path::new(&project), cycle, scans) {
        Ok(matched) => println!("{project}: {matched} values matched over {scans} scans"),
        Err(divergence) => panic!("{project}: {divergence}"),
    }
}
