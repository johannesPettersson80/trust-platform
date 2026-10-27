//! The plant demo's configuration: its task and the VAR_CONFIG bindings of the program instance's
//! variables to process-image addresses.
//!
//! The test needs the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`.

use trust_runtime::harness::TestHarness;
use trust_runtime::io::IoAddress;

include!("../../../tests/support/repository_source_oracle.rs");

#[test]
fn plant_demo_configuration_binds_io_and_tasks() {
    let repository_root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let root = repository_root.join("examples/plant_demo/src");
    let files = [
        "types.st",
        "fb_pump.st",
        "program.st",
        "config.st",
        "../libraries/hmi/src/hmi.st",
    ];
    let mut sources = Vec::with_capacity(files.len());
    for file in files {
        let path = root.join(file);
        let source = repository_source_tree_read_to_string!(
            (&path, &repository_root),
            roots = ["examples/plant_demo"],
            extension = "st",
        )
        .unwrap_or_else(|err| panic!("failed to read {}: {err}", path.display()));
        sources.push(source);
    }
    let source_refs: Vec<&str> = sources.iter().map(|source| source.as_str()).collect();

    let harness = TestHarness::from_sources(&source_refs).expect("compile plant_demo sources");
    let runtime = harness.runtime();
    assert!(
        !runtime.tasks().is_empty(),
        "expected CONFIGURATION TASK entries to be registered"
    );
    for (variable, address) in [
        ("P1.StartCmd", "%IX0.0"),
        ("P1.SpeedRaw", "%IW0"),
        ("P1.HaltReq", "%QX0.0"),
    ] {
        let symbol = runtime
            .find_variable(variable)
            .unwrap_or_else(|error| panic!("{variable}: {error}"));
        assert_eq!(
            runtime.bound_addresses(symbol),
            [IoAddress::parse(address).expect("a process-image address")],
            "expected VAR_CONFIG to bind {variable} at {address}"
        );
    }
}
