//! The reference workload of specification 44 §14.1, the vat pasteurizer, compiled into a native
//! module for this computer and run by the kernel: its image carries the project's deployment
//! record, and it runs its batches to completion against the plant simulation.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use std::path::Path;

use trust_runtime::bundle_builder::collect_project_source_files;
use trust_runtime::native::{build, load, Device, NoHooks, Options, Target, Toolchain};
use trust_runtime::value::{Duration, Value};
use trust_runtime_core::deployment::{DeploymentRecord, DeviceDescriptor, ProgramVersion};
use trust_runtime_services::installation::{admit_sequenced, encode_image, ProgramFormat};

const MS: i64 = 1_000_000;

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
fn the_pasteurizer_runs_natively_and_completes_its_batches() {
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
    // safe values. A device that runs images admits the record with the image; one that runs no
    // program format refuses it.
    let record = trust_runtime::deployment::image_record(&project, &built.image)
        .expect("the record of the image");
    let decoded = DeploymentRecord::decode(&record).expect("the record reads");
    assert_eq!(decoded.configuration.resource_name, "Plc");
    assert_eq!(decoded.configuration.cycle_interval_nanos, 10 * MS as u64);
    assert_eq!(decoded.configuration.safe_state.len(), 3);
    assert_eq!(decoded.tasks.len(), 3);
    let container = encode_image(1, &built.image, &record).expect("the container encodes");
    let images_only = DeviceDescriptor {
        program_versions: &[ProgramVersion::NATIVE_IMAGE],
        ..DeviceDescriptor::hosted()
    };
    let admitted = admit_sequenced(&container, &images_only).expect("the device admits the image");
    assert_eq!(admitted.admitted.format, ProgramFormat::Image);
    assert_eq!(admitted.admitted.program, built.image.as_slice());
    let no_format = DeviceDescriptor {
        program_versions: &[],
        ..DeviceDescriptor::hosted()
    };
    assert!(admit_sequenced(&container, &no_format).is_err());

    let mut program = load(&built.image, &device()).expect("the image loads");
    program.cold_start(Duration::ZERO).expect("the cold start");

    // An hour of scans: the vat fills, heats, holds and cools, and batches complete, including the
    // one the simulated contactor drop aborts and the operator resets.
    for scan in 1..=360_000i64 {
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
