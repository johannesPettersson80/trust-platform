//! A project compiled into a native module and run by the kernel computes what today's runtime
//! computes, variable for variable, scan after scan (specification 44 §9, §14.2): both run the
//! project's configuration at its cycle interval in lockstep, and after every scan each variable
//! the image's symbol table names a value of is compared with the runtime's.
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::TestHarness;
use trust_runtime::native::{build, load, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::native::image::{SymbolRecord, SYMBOL_PROGRAM};

/// Whether the kernel's `native` is the runtime's `runtime`: the same bits for a real, inside an
/// array or a structure too; the same value otherwise.
fn same(native: &Value, runtime: &Value) -> bool {
    match (native, runtime) {
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        (Value::LReal(a), Value::LReal(b)) => a.to_bits() == b.to_bits(),
        (Value::Array(a), Value::Array(b)) => {
            a.dimensions() == b.dimensions()
                && a.elements().len() == b.elements().len()
                && a.elements()
                    .iter()
                    .zip(b.elements())
                    .all(|(a, b)| same(a, b))
        }
        (Value::Struct(a), Value::Struct(b)) => {
            a.type_name() == b.type_name()
                && a.fields().len() == b.fields().len()
                && a.fields()
                    .iter()
                    .all(|(name, a)| b.field(name).is_some_and(|b| same(a, b)))
        }
        (a, b) => a == b,
    }
}

/// Runs the project in `project` for `scans` scans of its cycle interval on both, and returns the
/// first variable whose values differ, with the scan and both values, or how many values matched.
fn compare(project: &Path, cycle_ms: i64, scans: i64) -> Result<usize, String> {
    let toolchain = Toolchain::from_env()
        .map_err(|error| format!("TRUST_TOOLCHAIN names no toolchain: {error}"))?;
    let host = Target::this_cpu().ok_or("this computer's CPU has no native target")?;
    let sources = collect_project_source_files(project, None).map_err(|error| error.to_string())?;
    let work = std::env::temp_dir().join(format!(
        "trust-native-compare-{}-{}",
        std::process::id(),
        project
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("project")
    ));
    let built = build(
        &sources,
        &Options {
            target: host,
            hooks: true,
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built = built.map_err(|error| format!("the project does not compile: {error}"))?;
    let device = Device {
        target: host,
        program_memory: 1 << 30,
        task_stack: 1 << 24,
        max_statements: 1_000_000,
    };
    let mut program = load(&built.image, &device).map_err(|error| error.to_string())?;
    program
        .cold_start(Duration::ZERO)
        .map_err(|error| error.to_string())?;
    let texts: Vec<&str> = sources.iter().map(|source| source.text.as_str()).collect();
    let mut runtime = TestHarness::from_sources(&texts).map_err(|error| error.to_string())?;

    // The variables: every symbol of a variable, the program instances themselves aside.
    let symbols: Vec<(u32, String)> = {
        let image = program.image();
        let table = image
            .table::<SymbolRecord>()
            .map_err(|error| error.to_string())?;
        (0u32..)
            .zip(table.iter())
            .filter(|(_, symbol)| symbol.flags & SYMBOL_PROGRAM == 0)
            .map(|(index, symbol)| {
                (
                    index,
                    image.string(symbol.display).unwrap_or("").to_string(),
                )
            })
            .collect()
    };
    let mut matched = 0;
    for scan in 1..=scans {
        let now = Duration::from_millis(scan * cycle_ms);
        let native = program.scan(now, &mut NoHooks);
        runtime.advance_time(Duration::from_millis(cycle_ms));
        let result = runtime.cycle();
        let native_errors: Vec<String> = native.err().map(|e| e.to_string()).into_iter().collect();
        let runtime_errors: Vec<String> = result.errors.iter().map(ToString::to_string).collect();
        if native_errors != runtime_errors {
            let fault = program.last_fault();
            return Err(format!(
                "scan {scan}: the kernel's errors {native_errors:?} (statement {}, detail {:?}), the runtime's {runtime_errors:?}",
                fault.stmt, fault.detail
            ));
        }
        for (index, name) in &symbols {
            // A symbol that holds no value of its own, an instance or a reference, is compared
            // through its members.
            let Ok(native) = program.read_symbol(*index) else {
                continue;
            };
            let runtime = runtime.get_output(name);
            if !runtime
                .as_ref()
                .is_some_and(|runtime| same(&native, runtime))
            {
                return Err(format!(
                    "scan {scan}: {name} is {native:?} compiled, {runtime:?} in the runtime"
                ));
            }
            matched += 1;
        }
    }
    Ok(matched)
}

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
