//! `trust-dev rust bench`: measures the project's Rust POUs and records the longest call of each in
//! `trust.toml` `[timing.measured]` (specification 48 RU-48).
//!
//! The bench builds the program with its crate and a test program for each case of each Rust
//! function, function block and method (`trust_runtime::native::rust_pous::bench`) into a debug
//! image, in which the kernel keeps each Rust POU's longest call (RU-49), and runs each case once a
//! scan on this computer; a Rust program's case writes its inputs into each of the program's
//! configured instances before each scan of it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;

use anyhow::{bail, Context};
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::collect_project_sources_labelled;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::rust_pous::{bench, measured, read};
use trust_runtime::native::{
    build_project, conversion_profile, load, this_computer, triple, Device, HostCode,
    NativeProgram, Options, Target, Toolchain,
};
use trust_runtime::value::Duration;

/// The scans each case runs by default (RU-48).
pub const DEFAULT_SCANS: u32 = 1_000;

/// The statements one scan of a case may run, as a test's (specification 44 §8.4).
const BENCH_STATEMENTS: u32 = 100_000_000;

/// The logical time between two scans of a case: a task of 1 ms.
const SCAN_NS: i64 = 1_000_000;

/// `trust-dev rust bench`: measures every case of the project's Rust POUs on this computer and
/// writes each POU's longest call into `trust.toml` `[timing.measured]`.
pub fn run_bench(project: Option<PathBuf>, scans: u32) -> anyhow::Result<()> {
    let project = match project {
        Some(path) => path,
        None => match detect_bundle_path(None) {
            Ok(path) => path,
            Err(_) => std::env::current_dir().context("failed to resolve current directory")?,
        },
    };
    if scans == 0 {
        bail!("--scans takes at least one scan");
    }
    let krate = read(&project).map_err(|errors| {
        anyhow::anyhow!(errors
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join("\n"))
    })?;
    let Some(krate) = krate else {
        println!("The project declares no Rust POU.");
        return Ok(());
    };
    let plan = bench::programs(&krate);
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the bench needs the toolchain: {error}"))?;
    let host = Target::this_cpu().context("this computer's CPU has no native target")?;
    let conversions = conversion_profile(&project)?;
    let mut sources = collect_project_sources_labelled(&project, &[])?;
    sources.push(SourceFile::with_path("trust-bench.st", plan.source.clone()));
    let programs: Vec<SmolStr> = plan
        .programs
        .iter()
        .map(|program| program.program.clone())
        .collect();
    let work = std::env::temp_dir().join(format!("trust-dev-bench-{}", std::process::id()));
    let built = build_project(
        &project,
        &sources,
        &programs,
        &Options {
            target: host,
            hooks: true,
        },
        conversions,
        &toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    let built = built.map_err(|error| anyhow::anyhow!("the project does not compile: {error}"))?;
    let device = Device {
        max_statements: BENCH_STATEMENTS,
        ..this_computer().context("this computer's CPU runs no program image")?
    };
    let mut program = load(&built.image, &device)
        .map_err(|error| anyhow::anyhow!("the program image does not load: {error}"))?;
    let mut found = Found::default();
    for case in &plan.programs {
        let instance = built
            .instances
            .iter()
            .position(|instance| instance.eq_ignore_ascii_case(case.program.as_str()))
            .and_then(|index| u32::try_from(index).ok())
            .with_context(|| format!("the bench's test program {} was not built", case.program))?;
        if let Err(error) = scan(&mut program, &[(instance, "")], &[], scans) {
            found.fault(&case.pou, &case.case, &error);
        }
        found.take(&program);
        found.measured.insert(case.pou.clone());
    }
    for case in &plan.program_cases {
        let instances: Vec<(u32, &str)> = built
            .instances
            .iter()
            .zip(&built.instance_types)
            .zip(0u32..)
            .filter(|((_, ty), _)| ty.eq_ignore_ascii_case(&case.pou))
            .map(|((name, _), index)| (index, name.as_str()))
            .collect();
        if instances.is_empty() {
            found.unconfigured.insert(case.pou.clone());
            continue;
        }
        if let Err(error) = scan(&mut program, &instances, &case.inputs, scans) {
            found.fault(&case.pou, &case.case, &error);
        }
        found.take(&program);
        found.measured.insert(case.pou.clone());
    }
    let code =
        measured::code(&project, triple(host)).map_err(|message| anyhow::anyhow!(message))?;
    report(&project, host, scans, &found, &code)
}

/// What a bench's runs found.
#[derive(Default)]
struct Found {
    /// Each Rust POU's longest call and its budget for the target, by its name in the Rust POU
    /// table.
    longest: BTreeMap<String, (i64, i64)>,
    /// The cases that faulted, by their POU: the case and the fault.
    faulted: BTreeMap<String, Vec<String>>,
    /// The POUs a case ran: a `PRIVATE` or `PROTECTED` method has none, and so no entry (RU-48).
    measured: BTreeSet<String>,
    /// The Rust programs the image has no instance of. The build gives every program an instance,
    /// one the configuration declares or, without a configuration, its own, so none is expected.
    unconfigured: BTreeSet<String>,
}

impl Found {
    /// Takes each Rust POU's record after a case's run: the kernel keeps each one's longest call.
    fn take(&mut self, program: &NativeProgram<'_, HostCode>) {
        for (pou, time) in (0u32..).zip(program.rust_times().iter()) {
            let Some(name) = program.rust_pou_name(pou) else {
                continue;
            };
            let entry = self
                .longest
                .entry(name.to_string())
                .or_insert((0, time.budget_ns));
            entry.0 = entry.0.max(time.longest_ns);
        }
    }

    /// Records that `pou`'s case `case` ended with `error`.
    fn fault(&mut self, pou: &str, case: &str, error: &str) {
        let shown = if case.is_empty() {
            "of its declared initial values"
        } else {
            case
        };
        self.faulted
            .entry(pou.to_string())
            .or_default()
            .push(format!("{shown}: {error}"));
    }
}

/// Runs `scans` scans after a cold start: each scan writes `inputs` into each of `instances`, each
/// a program instance's number and name, and runs the instance's body once. A test program takes
/// no input.
fn scan(
    program: &mut NativeProgram<'_, HostCode>,
    instances: &[(u32, &str)],
    inputs: &[(String, String)],
    scans: u32,
) -> Result<(), String> {
    program
        .cold_start(Duration::ZERO)
        .map_err(|error| error.to_string())?;
    for scan in 0..i64::from(scans) {
        let now = Duration::from_nanos(scan * SCAN_NS);
        for (index, name) in instances {
            bench::write_inputs(program, name, inputs)?;
            program
                .run_instance(*index, now)
                .map_err(|error| error.to_string())?;
        }
    }
    Ok(())
}

/// Prints what the bench found and writes the measurements of this computer's target, each with
/// `code`, into `trust.toml`; fails when a case faulted.
fn report(
    project: &std::path::Path,
    host: Target,
    scans: u32,
    found: &Found,
    code: &str,
) -> anyhow::Result<()> {
    let mut records = Vec::new();
    println!(
        "Measured on this computer ({}), {scans} scan(s) a case:",
        triple(host)
    );
    for (pou, (time, budget)) in &found.longest {
        if !found.measured.contains(pou) {
            continue;
        }
        if let Some(faults) = found.faulted.get(pou) {
            for fault in faults {
                println!("  {pou}: the case {fault}; no time recorded");
            }
            continue;
        }
        let over = if time > budget {
            "  over its budget"
        } else {
            ""
        };
        println!("  {pou}: longest call {time} ns, budget {budget} ns{over}");
        records.push(measured::Measured {
            pou: pou.clone(),
            longest_ns: u64::try_from(*time).unwrap_or(0),
            code: code.to_string(),
            device: "computer".to_string(),
        });
    }
    for pou in &found.unconfigured {
        println!("  {pou}: the program image has no instance of it; not measured");
    }
    measured::write(project, triple(host), &records).map_err(|message| anyhow::anyhow!(message))?;
    println!(
        "Wrote {} measurement(s) into trust.toml [timing.measured].",
        records.len()
    );
    if !found.faulted.is_empty() {
        bail!(
            "{} Rust POU(s) faulted in a case and are not measured",
            found.faulted.len()
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "rust_bench_tests.rs"]
mod tests;
