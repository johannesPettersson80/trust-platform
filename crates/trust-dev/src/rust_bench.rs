//! `trust-dev rust bench`: measures the project's Rust POUs and records the longest call of each in
//! `trust.toml` `[timing.measured]` (specification 48 RU-48).
//!
//! The bench builds the program with its crate and a test program for each case of each Rust
//! function, function block and method (`trust_runtime::native::rust_pous::bench`) into a debug
//! image, in which each Rust POU's longest call is recorded (RU-49), and runs each case once a scan
//! on this computer or, with `--device`, on a device by stepped scans (specification 11 §6.8.1); a
//! Rust program's case writes its inputs into each of the program's instances before each scan of
//! it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use anyhow::{bail, Context};
use serde_json::{json, Map, Value as JsonValue};
use smol_str::SmolStr;
use trust_runtime::bundle::detect_bundle_path;
use trust_runtime::bundle_builder::collect_project_sources_labelled;
use trust_runtime::harness::SourceFile;
use trust_runtime::native::rust_pous::bench::{self, Bench};
use trust_runtime::native::rust_pous::{measured, read};
use trust_runtime::native::{
    build_project, conversion_profile, load, target_named, this_computer, triple, Build, Device,
    HostCode, NativeProgram, Options, Target, Toolchain,
};
use trust_runtime::value::Duration;

/// The scans each case runs by default (RU-48).
pub const DEFAULT_SCANS: u32 = 1_000;

/// The statements one scan of a case may run, as a test's (specification 44 §8.4).
const BENCH_STATEMENTS: u32 = 100_000_000;

/// The logical time between two scans of a case: a task of 1 ms.
const SCAN_NS: i64 = 1_000_000;

/// The device a bench runs on (`--device`, `--target`, `--token`), as `trust-dev test` names one.
pub struct OnDevice {
    /// The device's serial port, or `tcp://<host>:<port>` of a bridge to it.
    pub port: String,
    /// The device's target, as `trust-runtime build --target` names it: `esp32c6`.
    pub target: String,
    /// The device's control token, when its settings set one.
    pub token: Option<String>,
}

/// `trust-dev rust bench`: measures every case of the project's Rust POUs on this computer, or on
/// `device`, and writes each POU's longest call into `trust.toml` `[timing.measured]` for the
/// target it ran on.
pub fn run_bench(
    project: Option<PathBuf>,
    scans: u32,
    device: Option<OnDevice>,
) -> anyhow::Result<()> {
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
        outln!("The project declares no Rust POU.");
        return Ok(());
    };
    let plan = bench::programs(&krate);
    let toolchain = Toolchain::find()
        .map_err(|error| anyhow::anyhow!("the bench needs the toolchain: {error}"))?;
    let (target, found, measured_on) = match &device {
        None => {
            let host = Target::this_cpu().context("this computer's CPU has no native target")?;
            let built = build_bench(&project, &plan, host, &toolchain)?;
            (
                host,
                bench_here(&plan, &built, scans)?,
                "computer".to_string(),
            )
        }
        Some(device) => {
            let target = target_named(&device.target)
                .ok_or_else(|| anyhow::anyhow!("unknown target '{}'", device.target))?;
            let mut built = build_bench(&project, &plan, target, &toolchain)?;
            // The device admits what a key it trusts signed: the project's (specification 44 §5.11).
            trust_runtime::native::sign(
                &mut built.image,
                &trust_runtime::native::project_key(&project)?,
            )?;
            let (found, identity) =
                bench_on(&project, &plan, &built, target, &toolchain, scans, device)?;
            (target, found, identity)
        }
    };
    let code =
        measured::code(&project, triple(target)).map_err(|message| anyhow::anyhow!(message))?;
    report(&project, target, &measured_on, scans, &found, &code)
}

/// The bench's debug image of the project for `target`: the program with the test programs of
/// `plan`.
fn build_bench(
    project: &Path,
    plan: &Bench,
    target: Target,
    toolchain: &Toolchain,
) -> anyhow::Result<Build> {
    let conversions = conversion_profile(project)?;
    let mut sources = collect_project_sources_labelled(project, &[])?;
    sources.push(SourceFile::with_path("trust-bench.st", plan.source.clone()));
    let programs: Vec<SmolStr> = plan
        .programs
        .iter()
        .map(|program| program.program.clone())
        .collect();
    let work = std::env::temp_dir().join(format!("trust-dev-bench-{}", std::process::id()));
    let built = build_project(
        project,
        &sources,
        &programs,
        &Options {
            target,
            hooks: true,
        },
        conversions,
        toolchain,
        &work,
    );
    let _ = std::fs::remove_dir_all(&work);
    built.map_err(|error| anyhow::anyhow!("the project does not compile: {error}"))
}

/// The number of the instance that runs the test program `program`.
fn test_program(built: &Build, program: &str) -> anyhow::Result<u32> {
    built
        .instances
        .iter()
        .position(|instance| instance.eq_ignore_ascii_case(program))
        .and_then(|index| u32::try_from(index).ok())
        .with_context(|| format!("the bench's test program {program} was not built"))
}

/// The instances of the Rust program `pou`, each by its number and its name.
fn instances_of<'b>(built: &'b Build, pou: &str) -> Vec<(u32, &'b str)> {
    built
        .instances
        .iter()
        .zip(&built.instance_types)
        .zip(0u32..)
        .filter(|((_, ty), _)| ty.eq_ignore_ascii_case(pou))
        .map(|((name, _), index)| (index, name.as_str()))
        .collect()
}

/// Runs the bench's cases on this computer.
fn bench_here(plan: &Bench, built: &Build, scans: u32) -> anyhow::Result<Found> {
    let device = Device {
        max_statements: BENCH_STATEMENTS,
        ..this_computer().context("this computer's CPU runs no program image")?
    };
    let mut program = load(&built.image, &device)
        .map_err(|error| anyhow::anyhow!("the program image does not load: {error}"))?;
    let mut found = Found::default();
    for case in &plan.programs {
        let instance = test_program(built, &case.program)?;
        if let Err(error) = scan(&mut program, &[(instance, "")], &[], scans) {
            found.fault(&case.pou, &case.case, &error);
        }
        found.take_program(&program);
        found.measured.insert(case.pou.clone());
    }
    for case in &plan.program_cases {
        let instances = instances_of(built, &case.pou);
        if instances.is_empty() {
            found.unconfigured.insert(case.pou.clone());
            continue;
        }
        if let Err(error) = scan(&mut program, &instances, &case.inputs, scans) {
            found.fault(&case.pou, &case.case, &error);
        }
        found.take_program(&program);
        found.measured.insert(case.pou.clone());
    }
    Ok(found)
}

/// Runs the bench's cases on `device` by stepped scans (specification 11 §6.8.1), after installing
/// the bench's image there as `trust-dev test --device` installs a project's tests; gives what it
/// found and the device's runtime ID, which its `status` reports.
fn bench_on(
    project: &Path,
    plan: &Bench,
    built: &Build,
    target: Target,
    toolchain: &Toolchain,
    scans: u32,
    device: &OnDevice,
) -> anyhow::Result<(Found, String)> {
    use trust_runtime::device_link::{self, DeviceLink};

    // A device whose settings set a control token refuses a request without it (specification 34
    // PORT-06): the caller's token, else the project's, which the device was installed with.
    // The caller's token is `--token`, else `TRUST_CTL_TOKEN` (specification 22).
    let from_env = trust_runtime::control::endpoint::control_token_from_env();
    let token = trust_runtime::control::endpoint::device_token(
        Some(project),
        device.token.as_deref().or(from_env.as_deref()),
    )?;
    let mut link = DeviceLink::open(&device.port, token.as_deref())?;
    // Which update the device and these tools need (specification 34 TARGET-05).
    if let Some(advice) = link.version_advice() {
        eprintln!("warning: {advice}");
    }
    let work = std::env::temp_dir().join(format!("trust-dev-bench-device-{}", std::process::id()));
    let prepared = link
        .prepare_steps(target, toolchain, &work.join("idle"))
        .and_then(|()| device_link::stepping_container(&built.image, &work.join("bench")))
        .and_then(|container| link.install(&container));
    let _ = std::fs::remove_dir_all(&work);
    prepared.map_err(|error| anyhow::anyhow!("install the bench on the device: {error}"))?;
    let status = link
        .call("status", None, device_link::REPLY_WAIT)
        .map_err(|error| anyhow::anyhow!("the device's status: {error}"))?;
    let identity = status["runtime_id"]
        .as_str()
        .filter(|id| !id.is_empty())
        .unwrap_or("device")
        .to_string();
    let mut found = Found::default();
    // The stepped scans keep one logical time across the cases: no step is before the last one.
    let mut time_ns: u64 = 0;
    for case in &plan.programs {
        let run = vec![(case.program.to_string(), Map::new())];
        match step(&mut link, &run, scans, &mut time_ns) {
            Ok(records) => found.take_records(&records),
            Err(error) => found.fault(&case.pou, &case.case, &error),
        }
        found.measured.insert(case.pou.clone());
    }
    for case in &plan.program_cases {
        let instances = instances_of(built, &case.pou);
        if instances.is_empty() {
            found.unconfigured.insert(case.pou.clone());
            continue;
        }
        let runs: Result<Vec<Run>, String> = instances
            .iter()
            .map(|(_, name)| {
                let inputs = bench::step_inputs(&built.image, name, &case.inputs)?;
                Ok((
                    (*name).to_string(),
                    inputs
                        .into_iter()
                        .map(|(path, text)| (path, JsonValue::String(text)))
                        .collect(),
                ))
            })
            .collect();
        match runs.and_then(|runs| step(&mut link, &runs, scans, &mut time_ns)) {
            Ok(records) => found.take_records(&records),
            Err(error) => found.fault(&case.pou, &case.case, &error),
        }
        found.measured.insert(case.pou.clone());
    }
    Ok((found, identity))
}

/// Steps `scans` scans of `runs`, each a program instance with the inputs written before it, the
/// first restarting the program cold at the stepped time `time_ns`, which goes on a scan a step;
/// gives the Rust POU records of the last step's reply.
fn step(
    link: &mut trust_runtime::device_link::DeviceLink,
    runs: &[Run],
    scans: u32,
    time_ns: &mut u64,
) -> Result<JsonValue, String> {
    let mut records = JsonValue::Null;
    let mut first = true;
    for _ in 0..scans {
        for (run, inputs) in runs {
            let mut params = json!({"time_ns": *time_ns, "run": run, "inputs": inputs});
            if first {
                params["restart"] = json!("cold");
                first = false;
            }
            let result = link
                .call(
                    "cycle.step",
                    Some(params),
                    trust_runtime::device_link::REPLY_WAIT,
                )
                .map_err(|error| error.to_string())?;
            if let Some(error) = result["errors"].get(0) {
                return Err(error.as_str().unwrap_or_default().to_string());
            }
            records = result["rust_pous"].clone();
            *time_ns = time_ns.saturating_add(SCAN_NS.unsigned_abs());
        }
    }
    if !records.is_array() {
        return Err(
            "the device's step reports no Rust POU records: update its truST to this version \
             (specification 11 §6.8.1)"
                .to_string(),
        );
    }
    Ok(records)
}

/// A program instance a step runs, with the inputs the step writes before its scan.
type Run = (String, Map<String, JsonValue>);

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
    /// Takes each Rust POU's record after a case's run on this computer: the kernel keeps each
    /// one's longest call.
    fn take_program(&mut self, program: &NativeProgram<'_, HostCode>) {
        for (pou, time) in (0u32..).zip(program.rust_times().iter()) {
            if let Some(name) = program.rust_pou_name(pou) {
                self.take(name, time.longest_ns, time.budget_ns);
            }
        }
    }

    /// Takes each Rust POU's record from a step's reply on a device, as `tasks.stats` lists it.
    fn take_records(&mut self, records: &JsonValue) {
        for record in records.as_array().into_iter().flatten() {
            if let (Some(name), Some(longest), Some(budget)) = (
                record["name"].as_str(),
                record["longest_ns"].as_i64(),
                record["budget_ns"].as_i64(),
            ) {
                self.take(name, longest, budget);
            }
        }
    }

    fn take(&mut self, name: &str, longest_ns: i64, budget_ns: i64) {
        let entry = self
            .longest
            .entry(name.to_string())
            .or_insert((0, budget_ns));
        entry.0 = entry.0.max(longest_ns);
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

/// Runs `scans` scans after a cold start on this computer: each scan writes `inputs` into each of
/// `instances`, each a program instance's number and name, and runs the instance's body once. A
/// test program takes no input.
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

/// Prints what the bench found and writes the measurements of `target`, each with `code` and the
/// device `measured_on`, into `trust.toml`; fails when a case faulted.
fn report(
    project: &Path,
    target: Target,
    measured_on: &str,
    scans: u32,
    found: &Found,
    code: &str,
) -> anyhow::Result<()> {
    let mut records = Vec::new();
    let place = if measured_on == "computer" {
        "this computer".to_string()
    } else {
        format!("the device {measured_on}")
    };
    outln!(
        "Measured on {place} ({}), {scans} scan(s) a case:",
        triple(target)
    );
    for (pou, (time, budget)) in &found.longest {
        if !found.measured.contains(pou) {
            continue;
        }
        if let Some(faults) = found.faulted.get(pou) {
            for fault in faults {
                outln!("  {pou}: the case {fault}; no time recorded");
            }
            continue;
        }
        let over = if time > budget {
            "  over its budget"
        } else {
            ""
        };
        outln!("  {pou}: longest call {time} ns, budget {budget} ns{over}");
        records.push(measured::Measured {
            pou: pou.clone(),
            longest_ns: u64::try_from(*time).unwrap_or(0),
            code: code.to_string(),
            device: measured_on.to_string(),
        });
    }
    for pou in &found.unconfigured {
        outln!("  {pou}: the program image has no instance of it; not measured");
    }
    measured::write(project, triple(target), &records)
        .map_err(|message| anyhow::anyhow!(message))?;
    outln!(
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
