//! The reference workload of specification 44 §14.1 at 1x on this computer: the vat pasteurizer,
//! compiled into a native module, scanned in real time on a fixed grid of its resource's cycle
//! interval (specification 10 §4.3), for `TRUST_REALTIME_SECONDS` seconds (one hour by default). It
//! reports each task's worst-case scan, the activations its tasks missed and the batches the plant
//! completed, the numbers each milestone reports for a computer.
//!
//! It runs only when asked (`cargo test -p trust-runtime --test native_vat_realtime -- --ignored
//! --nocapture`), with the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`. The project is
//! `TRUST_REALTIME_PROJECT` when set, a variant of the pasteurizer with other task intervals, and
//! the pasteurizer otherwise.

use std::path::{Path, PathBuf};
use std::time::{Duration as Wall, Instant};

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::native::{build, load, Device, Options, ScanHooks, Target, Toolchain};
use trust_runtime::value::{Duration, Value};

/// What one task did: its scans' durations and the activations it missed.
#[derive(Default)]
struct TaskRecord {
    name: String,
    scans: u64,
    total: Wall,
    worst: Wall,
    missed: u64,
    started: Option<Instant>,
}

/// Times each task of a scan on the wall clock.
struct Timing<'t>(&'t mut [TaskRecord]);

impl ScanHooks for Timing<'_> {
    fn task_overrun(&mut self, task: usize, missed: u64) {
        self.0[task].missed += missed;
    }

    fn task_started(&mut self, task: usize) {
        self.0[task].started = Some(Instant::now());
    }

    fn task_finished(&mut self, task: usize) {
        let record = &mut self.0[task];
        if let Some(started) = record.started.take() {
            let took = started.elapsed();
            record.scans += 1;
            record.total += took;
            record.worst = record.worst.max(took);
        }
    }
}

fn report(tasks: &[TaskRecord], lateness: Wall, cycles: u64) -> String {
    let mut text = format!("cycles {cycles}, worst cycle start lateness {lateness:?}");
    for task in tasks.iter().filter(|task| task.scans > 0) {
        let average = task.total / u32::try_from(task.scans).unwrap_or(u32::MAX);
        text.push_str(&format!(
            "; {} {} scans, average {average:?}, worst {:?}, missed {}",
            task.name, task.scans, task.worst, task.missed
        ));
    }
    text
}

#[test]
#[ignore = "an hour of real time; run on request"]
fn the_pasteurizer_runs_in_real_time() {
    let toolchain = Toolchain::from_env()
        .expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = std::env::var_os("TRUST_REALTIME_PROJECT").map_or_else(
        || Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/vat_pasteurizer"),
        PathBuf::from,
    );
    let seconds: u64 = std::env::var("TRUST_REALTIME_SECONDS")
        .ok()
        .and_then(|text| text.parse().ok())
        .unwrap_or(3_600);
    let target = Target::this_cpu().expect("a computer target");
    let sources = collect_project_source_files(&project, None).expect("the project's sources");
    let work = std::env::temp_dir().join(format!("trust-native-rt-{}", std::process::id()));
    let built = build(
        &sources,
        &Options {
            target,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the project compiles");
    let _ = std::fs::remove_dir_all(&work);
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000_000,
    };
    let mut program = load(&built.image, &device).expect("the image loads");
    let mut tasks: Vec<TaskRecord> = program
        .tasks()
        .map(|(name, _)| TaskRecord {
            name: name.to_string(),
            ..TaskRecord::default()
        })
        .collect();
    // The resource cycle is the shortest task interval, as the pasteurizer's runtime.toml sets it.
    let interval = Wall::from_millis(
        std::env::var("TRUST_REALTIME_CYCLE_MS")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(10),
    );
    println!(
        "{project:?} at 1x for {seconds} s, cycle {interval:?}, image {} bytes, figure {} bytes",
        built.image.len(),
        built.figure.total
    );
    program.cold_start(Duration::ZERO).expect("the cold start");
    let origin = Instant::now();
    let end = origin + Wall::from_secs(seconds);
    let mut boundary = origin;
    let mut lateness = Wall::ZERO;
    let mut cycles = 0u64;
    let mut next_report = origin + Wall::from_secs(60);
    loop {
        let now = Instant::now();
        if now < boundary {
            std::thread::sleep(boundary - now);
        }
        let started = Instant::now();
        lateness = lateness.max(started.saturating_duration_since(boundary));
        let logical = Duration::from_nanos(
            i64::try_from(started.duration_since(origin).as_nanos()).unwrap_or(i64::MAX),
        );
        program
            .scan(logical, &mut Timing(&mut tasks))
            .unwrap_or_else(|error| panic!("scan {cycles}: {error}"));
        cycles += 1;
        let finished = Instant::now();
        // The next boundary on the grid; a cycle longer than its interval starts the next one at
        // once and the grid goes on from it (specification 10 §4.3).
        boundary += interval;
        if boundary <= finished {
            boundary = finished;
        }
        if finished >= next_report {
            next_report += Wall::from_secs(60);
            println!(
                "{} min: {}, batches {:?}",
                finished.duration_since(origin).as_secs() / 60,
                report(&tasks, lateness, cycles),
                program.value("VAT.BATCHES")
            );
        }
        if finished >= end {
            break;
        }
    }
    println!("final: {}", report(&tasks, lateness, cycles));
    let batches = program.value("VAT.BATCHES");
    println!("batches: {batches:?}");
    assert!(
        matches!(batches, Some(Value::UDInt(count)) if u64::from(count) >= seconds / 600),
        "the plant completes about a batch every seven minutes: {batches:?} after {seconds} s"
    );
}
