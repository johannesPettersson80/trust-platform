//! Lockstep comparison of a compiled program with the runtime (specification 44 §9, §14.2): both
//! run the sources' configuration at a cycle interval, and after every scan each variable the
//! image's symbol table names a value of is compared with the runtime's, and so are the errors of
//! the scan. Shared by the native comparison tests.

use std::path::Path;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::{SourceFile, TestHarness};
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

/// Runs the project in `project` for `scans` scans of `cycle_ms` on both, and returns the first
/// variable whose values differ, with the scan and both values, or how many values matched.
pub fn compare(project: &Path, cycle_ms: i64, scans: i64) -> Result<usize, String> {
    let sources = collect_project_source_files(project, None).map_err(|error| error.to_string())?;
    let label = project
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project")
        .to_string();
    compare_sources(&label, &sources, cycle_ms, scans)
}

/// Runs `sources` for `scans` scans of `cycle_ms` on both, as [`compare`] does; `label` names its
/// work folder. The module is a debug image, or a release image when `TRUST_COMPARE_RELEASE` is
/// set (specification 44 §13.2), which holds the control variables of loops (§6.6).
pub fn compare_sources(
    label: &str,
    sources: &[SourceFile],
    cycle_ms: i64,
    scans: i64,
) -> Result<usize, String> {
    let toolchain = Toolchain::find()
        .map_err(|error| format!("TRUST_TOOLCHAIN names no toolchain: {error}"))?;
    let host = Target::this_cpu().ok_or("this computer's CPU has no native target")?;
    let work = std::env::temp_dir().join(format!(
        "trust-native-compare-{}-{label}",
        std::process::id()
    ));
    let built = build(
        sources,
        &Options {
            target: host,
            hooks: std::env::var_os("TRUST_COMPARE_RELEASE").is_none(),
        },
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built = built.map_err(|error| format!("the project does not compile: {error}"))?;
    let device = Device {
        target: host,
        program_memory: 1 << 30,
        // No more than a test's thread has, 2 MiB; a recursion's call depth spends it
        // (specification 44 §8.5).
        task_stack: 1 << 20,
        max_statements: 1_000_000,
        trusted_keys: &[],
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
        if !native_errors.is_empty() {
            // Both ended the scan with the same fault, which halts a resource; both restart warm,
            // as an operator restarts it, keeping the retained variables (specification 11 §6.7).
            program
                .warm_start(now)
                .map_err(|error| format!("scan {scan}: the kernel's warm restart: {error}"))?;
            runtime
                .restart(trust_runtime::RestartMode::Warm)
                .map_err(|error| format!("scan {scan}: the runtime's warm restart: {error}"))?;
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
