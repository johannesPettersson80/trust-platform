//! The scan crate graph that `trust-dev rust audit` checks against the RustSec advisory database
//! (specification 48 RU-79): read as the build reads it, from cargo's metadata of the build's
//! package, for every module target; and the same crates in the program image's crate table and
//! its deployment record (specification 44 §5.12, specification 34 §5.1.1 section 9).
//!
//! The tests need the toolchain of specification 44 §4.6 at `TRUST_TOOLCHAIN`, as
//! `scripts/native/build_toolchain_libraries.sh` builds it, with cargo and the SDK's sources.

mod rust_pous_support;

use rust_pous_support::{built, custom_project, manifest};
use trust_runtime::native::{scan_crate_graph, Toolchain};
use trust_runtime_core::deployment::{CrateSource, DeploymentRecord};
use trust_runtime_core::native::image::{CrateRecord, Image, SectionId};

/// A Rust function that calls the project's own path crate `helper`.
const CRATE: &str = "#![cfg_attr(not(test), no_std)]
#![forbid(unsafe_code)]

use trust_plc::prelude::*;

/// Its input plus one, by the helper.
#[trust_function(name = \"PLUS_ONE\")]
#[budget(\"1us\")]
pub fn plus_one(_cx: &mut Cx, a: i32) -> Result<i32, Fault> {
    Ok(helper::next(a))
}
";

const HELPER_MANIFEST: &str =
    "[package]\nname = \"helper\"\nversion = \"0.3.1\"\nedition = \"2024\"\n";

const HELPER: &str = "#![no_std]
#![forbid(unsafe_code)]

/// `a` plus one, wrapping.
pub fn next(a: i32) -> i32 {
    a.wrapping_add(1)
}
";

const PROGRAM: &str = "PROGRAM Line
VAR
    bumped : DINT;
END_VAR
bumped := PLUS_ONE(41);
END_PROGRAM

CONFIGURATION Probe
RESOURCE Probe ON PLC
    TASK Fast (INTERVAL := T#10ms, PRIORITY := 1);
    PROGRAM Main WITH Fast : Line;
END_RESOURCE
END_CONFIGURATION
";

/// The project with the crate `rules`, which calls its path crate `helper` 0.3.1.
fn helper_project(name: &str) -> std::path::PathBuf {
    custom_project(
        name,
        &[
            ("Cargo.toml", &manifest("helper = { path = \"helper\" }\n")),
            ("src/lib.rs", CRATE),
            ("helper/Cargo.toml", HELPER_MANIFEST),
            ("helper/src/lib.rs", HELPER),
        ],
        PROGRAM,
    )
}

#[test]
fn the_audited_graph_is_the_builds_without_the_projects_crate_and_the_generated_packages() {
    // RU-79: the crate `rules` depends on the SDK (trust-plc at this release's version, built
    // from trust-plc-lib, which libm's mathematics serves, RU-12) and on its path crate helper
    // 0.3.1. The audit checks every crate of that graph by name and version, but not the
    // project's crate itself nor the build's own packages, trust_module and layer 1. Catches a
    // graph without the project's dependencies (only the SDK's), one with the project's crate or
    // the generated package (which no advisory can name), and a version other than the one built.
    let root = helper_project("rust-audit-graph");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let graph = scan_crate_graph(&root, &toolchain, &root.join("build/audit"))
        .unwrap_or_else(|error| panic!("the graph reads: {error}"))
        .expect("the project's build compiles its crate");
    let names: Vec<&str> = graph.iter().map(|(name, _)| name.as_str()).collect();
    assert!(
        graph.contains(&("helper".to_string(), "0.3.1".to_string())),
        "{graph:?}"
    );
    assert!(
        graph.contains(&(
            "trust-plc".to_string(),
            env!("CARGO_PKG_VERSION").to_string()
        )),
        "{graph:?}"
    );
    assert!(names.contains(&"libm"), "{graph:?}");
    for generated in ["rules", "trust_module", "trust_st"] {
        assert!(!names.contains(&generated), "{generated} in {graph:?}");
    }
    // Once each, in order.
    let mut sorted = graph.clone();
    sorted.sort();
    sorted.dedup();
    assert_eq!(graph, sorted);
}

#[test]
fn a_project_without_a_crate_has_no_graph_to_check() {
    // RU-79: a project without a crate has nothing to check. Catches an audit that fails a
    // Structured Text project for lacking a crate.
    let root = trust_test_support::unique_temp_path("rust-audit-no-crate");
    std::fs::create_dir_all(root.join("src")).expect("the sources' folder");
    std::fs::write(root.join("src/main.st"), PROGRAM).expect("the program");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    assert_eq!(
        scan_crate_graph(&root, &toolchain, &root.join("build/audit"))
            .unwrap_or_else(|error| panic!("{error}")),
        None
    );
}

#[test]
fn the_image_and_its_record_list_the_crates_the_audit_checks() {
    // RU-79 and specification 44 §5.12: the built image lists the crates of its scan crate graph
    // for its target, crates the audit checks, in a crate table before SIGN, and its deployment
    // record carries the same list. helper is a path crate, so its checksum is zero; trust-plc is the SDK's, at this
    // release's version, also a path crate to the build (RU-75). Catches an image without the list,
    // a list other than the audit's, a list after the signature (unsigned), and a record that loses
    // it.
    let root = helper_project("rust-image-crates");
    let Some((build, _)) = built(&root) else {
        return;
    };
    let helper = build
        .crates
        .iter()
        .find(|entry| entry.name == "helper")
        .expect("helper is listed");
    assert_eq!(
        (helper.version.as_str(), helper.source, helper.checksum),
        ("0.3.1", CrateSource::Path, [0; 32])
    );
    assert!(
        build.crates.iter().any(|entry| entry.name == "trust-plc"
            && entry.version == env!("CARGO_PKG_VERSION")
            && entry.source == CrateSource::Path),
        "{:?}",
        build.crates
    );
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let audited = scan_crate_graph(&root, &toolchain, &root.join("build/audit"))
        .unwrap_or_else(|error| panic!("the graph reads: {error}"))
        .expect("the project's build compiles its crate");
    // The audit checks the graphs of every module target; this computer's image lists its own.
    for entry in &build.crates {
        let crate_name = (entry.name.to_string(), entry.version.to_string());
        assert!(
            audited.contains(&crate_name),
            "{crate_name:?} is not in the audit's graph {audited:?}"
        );
    }

    let image = Image::parse(&build.image).expect("the image reads");
    let table = image.section(SectionId::Crates).expect("the crate table");
    let sign = image.section(SectionId::Sign).expect("the signature");
    assert!(table.offset < sign.offset, "the table lies before SIGN");
    assert_eq!(
        image.table::<CrateRecord>().expect("the table reads").len() as usize,
        build.crates.len()
    );
    let record = DeploymentRecord::for_image(&build.image, [0; 32]).expect("the record");
    assert_eq!(record.crates, build.crates);
    let decoded = DeploymentRecord::decode(&record.encode().expect("the record encodes"))
        .expect("the record decodes");
    assert_eq!(decoded.crates, build.crates);
}
