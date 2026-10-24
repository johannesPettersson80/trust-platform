//! The reference workload of specification 44 §14.1, the vat pasteurizer, compiled into a native
//! module for this computer and run by the kernel: it computes what today's runtime computes, bit
//! for bit, scan after scan (NX-04, NX-12), and runs its batches to completion against the plant
//! simulation.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::harness::TestHarness;
use trust_runtime::native::{build, load, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::bytecode::BytecodeVersion;
use trust_runtime_core::deployment::{DeploymentRecord, DeviceDescriptor};
use trust_runtime_services::installation::{admit_sequenced, encode_image, ProgramFormat};

const MS: i64 = 1_000_000;

/// The variables compared, by the kernel's canonical name and the runtime's path.
const COMPARED: [(&str, &str); 9] = [
    ("VAT.STATE", "Vat.State"),
    ("VAT.BATCHES", "Vat.Batches"),
    ("STATUS.TEMPERATURE", "Status.Temperature"),
    ("STATUS.LEVEL", "Status.Level"),
    ("VAT.HEATEROUTPUT", "Vat.HeaterOutput"),
    ("IO.HEATERPOWER", "Io.HeaterPower"),
    ("PLANT.TEMP", "Plant.Temp"),
    ("STATUS.STATETEXT", "Status.StateText"),
    ("STATUS.ACTIVEALARMS", "Status.ActiveAlarms"),
];

/// Whether the kernel's `native` is the runtime's `runtime`: the same bits for a real, the same
/// value otherwise, an enumeration of the same type and value.
fn same(native: &Value, runtime: &Value) -> bool {
    match (native, runtime) {
        (Value::Real(a), Value::Real(b)) => a.to_bits() == b.to_bits(),
        (Value::LReal(a), Value::LReal(b)) => a.to_bits() == b.to_bits(),
        (a, b) => a == b,
    }
}

fn device() -> Device {
    Device {
        target: Target::X86_64,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000_000,
        trusted_keys: &[],
    }
}

#[test]
fn the_pasteurizer_runs_natively_as_the_runtime_runs_it() {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let project = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples/vat_pasteurizer");
    let sources = collect_project_source_files(&project, None).expect("the project's sources");
    let work = std::env::temp_dir().join(format!("trust-native-vat-{}", std::process::id()));
    let built = build(
        &sources,
        &Options {
            target: Target::X86_64,
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("the pasteurizer compiles");
    let _ = std::fs::remove_dir_all(&work);
    println!(
        "image {} bytes, code {}, memory figure {}",
        built.image.len(),
        built.figure.code,
        built.figure.total
    );
    // The image's deployment record carries the project's configuration (specification 44 §10.1):
    // `runtime.toml` names the resource `Plc` with a 10 ms cycle, and `io.toml` has the three lamps'
    // safe values. A device that runs images admits the record with the image; one that runs STBC
    // only refuses it.
    let record = trust_runtime::deployment::image_record(&project, &built.image)
        .expect("the record of the image");
    let decoded = DeploymentRecord::decode(&record).expect("the record reads");
    assert_eq!(decoded.configuration.resource_name, "Plc");
    assert_eq!(decoded.configuration.cycle_interval_nanos, 10 * MS as u64);
    assert_eq!(decoded.configuration.safe_state.len(), 3);
    assert_eq!(decoded.tasks.len(), 3);
    let container = encode_image(1, &built.image, &record).expect("the container encodes");
    let images_only = DeviceDescriptor {
        bytecode_versions: &[BytecodeVersion::NATIVE_IMAGE],
        ..DeviceDescriptor::hosted()
    };
    let admitted = admit_sequenced(&container, &images_only).expect("the device admits the image");
    assert_eq!(admitted.admitted.format, ProgramFormat::Image);
    assert_eq!(admitted.admitted.program, built.image.as_slice());
    assert!(admit_sequenced(&container, &DeviceDescriptor::hosted()).is_err());

    let mut program = load(&built.image, &device()).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");

    let texts: Vec<&str> = sources.iter().map(|source| source.text.as_str()).collect();
    let mut runtime = TestHarness::from_sources(&texts).expect("the runtime builds the project");

    // Ten minutes of scans side by side: the vat fills, heats, holds and cools.
    for scan in 1..=60_000i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .unwrap_or_else(|error| panic!("native scan {scan}: {error}"));
        runtime.advance_time(Duration::from_millis(10));
        let result = runtime.cycle();
        assert!(
            result.errors.is_empty(),
            "runtime scan {scan}: {:?}",
            result.errors
        );
        if scan % 10 == 0 {
            for (native_name, runtime_name) in COMPARED {
                let native = program
                    .value(native_name)
                    .unwrap_or_else(|| panic!("{native_name}"));
                let expected = runtime
                    .get_output(runtime_name)
                    .unwrap_or_else(|| panic!("{runtime_name}"));
                assert!(
                    same(&native, &expected),
                    "scan {scan}: {native_name} is {native:?}, the runtime has {expected:?}"
                );
            }
        }
    }

    // Then the compiled program alone, to an hour: batches complete, including the one the
    // simulated contactor drop aborts and the operator resets.
    for scan in 60_001..=360_000i64 {
        program
            .scan(Duration::from_nanos(scan * 10 * MS), &mut NoHooks)
            .unwrap_or_else(|error| panic!("native scan {scan}: {error}"));
        if scan % 6_000 == 0 {
            let show = |name: &str| format!("{name}={:?}", program.value(name));
            println!(
                "{} min: {} {} {} {} {} {} {}",
                scan / 6_000,
                show("VAT.STATE"),
                show("STATUS.LEVEL"),
                show("STATUS.TEMPERATURE"),
                show("STATUS.STEPTIME"),
                show("STATUS.FIRSTOUT"),
                show("STATUS.STATETEXT"),
                show("STATUS.ALARMTEXT"),
            );
        }
    }
    let batches = program.value("VAT.BATCHES");
    println!("batches after an hour: {batches:?}");
    let Some(Value::UDInt(batches)) = batches else {
        panic!("VAT.BATCHES is not a UDINT: {batches:?}");
    };
    assert!(
        batches >= 3,
        "only {batches} batches completed in 40 minutes"
    );
}
