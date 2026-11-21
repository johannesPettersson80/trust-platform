//! `trust-dev rust bench`: measures the project's Rust POUs and records the longest call of each in
//! `trust.toml` `[timing.measured]` (specification 48 RU-48).
//!
//! The bench builds the program with its crate and a test program for each case of each Rust
//! function, function block and method (`trust_runtime::native::rust_pous::bench`) into a debug
//! image, in which the kernel keeps each Rust POU's longest call (RU-49), and runs each case once a
//! scan on this computer.

use std::collections::BTreeMap;
use std::path::PathBuf;

use anyhow::{bail, Context};
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::collect_project_sources_labelled;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::rust_pous::{bench, measured, read};
use trust_runtime::native::{
    build_project, conversion_profile, load, this_computer, triple, Device, Options, Target,
    Toolchain,
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
    if plan.programs.is_empty() {
        println!("The project declares no Rust function, function block or method to measure.");
        return Ok(());
    }
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
    let mut longest: BTreeMap<String, (i64, i64)> = BTreeMap::new();
    let mut faulted: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for case in &plan.programs {
        let instance = built
            .instances
            .iter()
            .position(|instance| instance.eq_ignore_ascii_case(case.program.as_str()))
            .and_then(|index| u32::try_from(index).ok())
            .with_context(|| format!("the bench's test program {} was not built", case.program))?;
        let mut run = program.cold_start(Duration::ZERO);
        for scan in 0..i64::from(scans) {
            if run.is_err() {
                break;
            }
            run = program.run_instance(instance, Duration::from_nanos(scan * SCAN_NS));
        }
        if let Err(error) = run {
            let shown = if case.case.is_empty() {
                "its declared initial values".to_string()
            } else {
                case.case.clone()
            };
            faulted
                .entry(case.pou.clone())
                .or_default()
                .push(format!("{shown}: {error}"));
        }
        for (pou, time) in (0u32..).zip(program.rust_times().iter()) {
            let Some(name) = program.rust_pou_name(pou) else {
                continue;
            };
            let entry = longest
                .entry(name.to_string())
                .or_insert((0, time.budget_ns));
            entry.0 = entry.0.max(time.longest_ns);
        }
    }
    // Only the POUs the bench calls are measured: a PRIVATE or PROTECTED method has no entry (RU-48).
    let measured_pous: std::collections::BTreeSet<&str> = plan
        .programs
        .iter()
        .map(|program| program.pou.as_str())
        .collect();
    let code =
        measured::code(&project, triple(host)).map_err(|message| anyhow::anyhow!(message))?;
    let mut records = Vec::new();
    println!(
        "Measured on this computer ({}), {scans} scan(s) a case:",
        triple(host)
    );
    for (pou, (time, budget)) in &longest {
        if !measured_pous.contains(pou.as_str()) {
            continue;
        }
        if let Some(faults) = faulted.get(pou) {
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
            code: code.clone(),
            device: "computer".to_string(),
        });
    }
    measured::write(&project, triple(host), &records)
        .map_err(|message| anyhow::anyhow!(message))?;
    println!(
        "Wrote {} measurement(s) into trust.toml [timing.measured].",
        records.len()
    );
    if !faulted.is_empty() {
        bail!(
            "{} Rust POU(s) faulted in a case and are not measured",
            faulted.len()
        );
    }
    Ok(())
}

#[cfg(test)]
#[path = "rust_bench_tests.rs"]
mod tests;
