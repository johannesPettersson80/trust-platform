//! The program of an isolated run of a project with a Rust crate (protocol annex E8.2, E8.3): the
//! snapshot's crate files and sources are built as a project of their own, and the crate's function
//! computes in the run's scans. The test needs the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`, with cargo and the SDK's sources.

use super::super::crate_files;
use super::super::snapshot::{Snapshot, Source};
use super::super::worker::BuildSetting;
use super::IsolatedProgram;
use crate::harness::SourceFile;
use crate::native::Toolchain;
use crate::stdlib::conversions::ConversionProfile;
use crate::value::{Duration, Value};

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

fn write(root: &std::path::Path, path: &str, text: &str) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(path, text).expect("the file");
}

#[test]
fn an_isolated_run_builds_the_projects_rust_crate() {
    // E8.2 and E8.3: the run of a project with a Rust crate is built with the crate's files of its
    // snapshot, the crate's path dependency `helper` among them, and the generated declarations
    // that the build checks against the crate where they lie in the project. Main's task runs one
    // interval after the start, and its call of the Rust function PLUS_ONE(41) gives 42. Catches a
    // run built from the Structured Text alone, which knows no PLUS_ONE, and one whose copy lacks
    // the path crate or the declarations.
    let root = trust_test_support::unique_temp_path("isolated-rust-project");
    write(
        &root,
        "rust/Cargo.toml",
        &format!(
            "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
             [dependencies]\ntrust-plc = \"={}\"\nhelper = {{ path = \"helper\" }}\n",
            env!("CARGO_PKG_VERSION")
        ),
    );
    write(&root, "rust/src/lib.rs", CRATE);
    write(
        &root,
        "rust/helper/Cargo.toml",
        "[package]\nname = \"helper\"\nversion = \"0.3.1\"\nedition = \"2024\"\n",
    );
    write(&root, "rust/helper/src/lib.rs", HELPER);
    write(&root, "src/main.st", PROGRAM);
    crate::native::rust_pous::write_declarations(&root).expect("the declarations are written");
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        return;
    }
    let layout = crate::native::rust_pous::layout(&root).expect("the layout");
    let sources: Vec<Source> = ["src/main.st".to_string(), layout.declarations]
        .iter()
        .map(|path| Source {
            path: root.join(path).to_string_lossy().into_owned(),
            text: std::fs::read_to_string(root.join(path)).expect("the source"),
        })
        .collect();
    let snapshot = Snapshot {
        project_revision: String::new(),
        project_identity: root.to_string_lossy().into_owned(),
        sources: sources.clone(),
        simulation: String::new(),
        process_inputs: Vec::new(),
        process_inputs_revision: String::new(),
        conversion_profile: String::new(),
        crate_files: crate_files::read(&root).expect("the crate's files"),
    };
    let files: Vec<SourceFile> = sources
        .iter()
        .map(|source| SourceFile::with_path(&source.path, &source.text))
        .collect();
    let build = BuildSetting {
        toolchain: None,
        temporary_folder: Some(trust_test_support::unique_temp_path("isolated-rust-work")),
    };
    let mut program = IsolatedProgram::compile(&files, &snapshot, ConversionProfile::Iec, &build)
        .unwrap_or_else(|error| panic!("the crate's project builds isolated: {error}"));
    for ms in [0, 10] {
        program
            .runtime_mut()
            .set_current_time(Duration::from_millis(ms));
        program
            .runtime_mut()
            .execute_cycle()
            .unwrap_or_else(|error| panic!("the scan at {ms} ms runs: {error}"));
    }
    assert_eq!(
        program
            .runtime()
            .read_variable("Main.bumped")
            .unwrap_or_else(|error| panic!("Main.bumped reads: {error}")),
        Value::DInt(42)
    );
}
