//! Plant library specification §13 and §14.12: every runnable example under
//! `libraries/plant/examples`, the Plant tank example `examples/hmi_equipment`, and every complete
//! Structured Text example of the Plant manual (`docs/guides/PLANT_LIBRARY_GUIDE.md`, `iecst`
//! blocks declaring a PROGRAM or FUNCTION_BLOCK and not marked `fragment`) compiles with the real
//! library and runs ten scans without a runtime error.

mod plant_support;

use std::path::{Path, PathBuf};

use plant_support::{PLANT_SOURCE_FILES, SOURCES};
use trust_runtime::harness::TestHarness;
use trust_runtime::value::Duration;

/// The screens' block, which `examples/hmi_equipment` calls in its screen task.
const HMI: &str = include_str!("../../../libraries/hmi/src/hmi.st");

/// The example projects under `libraries/plant/examples` (§13.1).
const EXAMPLES: &[&str] = &[
    "crusher_lubrication",
    "equipment",
    "packaging_machine",
    "sequences",
    "wastewater_station",
];

/// One step is at least as long as every example task's interval, so every task runs each scan.
const STEP_MS: i64 = 250;

/// Compiled with a manual example that declares function blocks but no program.
const STUB_PROGRAM: &str = r#"
PROGRAM PlantManualExample
VAR
    Unused : BOOL;
END_VAR
END_PROGRAM
"#;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// The Structured Text files of a folder, in name order.
fn structured_text_files(folder: &Path) -> Vec<String> {
    let mut paths: Vec<PathBuf> = std::fs::read_dir(folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .map(|entry| entry.expect("a folder entry").path())
        .filter(|path| path.extension().is_some_and(|extension| extension == "st"))
        .collect();
    paths.sort();
    paths
        .iter()
        .map(|path| {
            std::fs::read_to_string(path)
                .unwrap_or_else(|error| panic!("{}: {error}", path.display()))
        })
        .collect()
}

/// Compiles a project with the library and runs ten scans.
fn compile_and_run(project: &str, files: &[String], libraries: &[&str]) {
    assert!(!files.is_empty(), "{project} has Structured Text sources");
    let mut sources: Vec<&str> = SOURCES.to_vec();
    sources.extend_from_slice(libraries);
    sources.extend(files.iter().map(String::as_str));
    let mut harness = TestHarness::from_sources(&sources)
        .unwrap_or_else(|error| panic!("{project} compiles with the Plant library: {error}"));
    for scan in 1..=10 {
        harness.advance_time(Duration::from_millis(STEP_MS));
        let result = harness.cycle();
        assert!(
            result.errors.is_empty(),
            "{project}: scan {scan} runs without a runtime error: {:?}",
            result.errors
        );
    }
}

fn plant_example(name: &str) {
    let folder = repository()
        .join("libraries/plant/examples")
        .join(name)
        .join("src");
    compile_and_run(name, &structured_text_files(&folder), &[]);
}

/// Whether a line of the source starts with one of the keywords.
fn declares(source: &str, keywords: &[&str]) -> bool {
    source.lines().any(|line| {
        let line = line.trim_start().to_ascii_uppercase();
        keywords
            .iter()
            .any(|keyword| line.starts_with(&format!("{keyword} ")))
    })
}

/// The complete examples of the manual: its `iecst` code blocks that declare a PROGRAM or a
/// FUNCTION_BLOCK, except blocks marked `fragment`.
fn complete_examples(manual: &str) -> Vec<String> {
    manual
        .split("```iecst")
        .skip(1)
        .filter_map(|rest| {
            let (info, code) = rest.split_once('\n')?;
            let code = code.split("```").next()?;
            let fragment = info.split_whitespace().any(|word| word == "fragment");
            (!fragment && declares(code, &["PROGRAM", "FUNCTION_BLOCK"])).then(|| code.to_owned())
        })
        .collect()
}

#[test]
fn every_plant_example_project_is_tested_here() {
    let folder = repository().join("libraries/plant/examples");
    let mut found: Vec<String> = std::fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .map(|entry| entry.expect("a folder entry"))
        .filter(|entry| entry.path().is_dir())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort();
    assert_eq!(found, EXAMPLES, "each example project has a test below");
}

#[test]
fn the_crusher_lubrication_example_compiles_and_runs() {
    plant_example("crusher_lubrication");
}

#[test]
fn the_equipment_example_compiles_and_runs() {
    plant_example("equipment");
}

#[test]
fn the_packaging_machine_example_compiles_and_runs() {
    plant_example("packaging_machine");
}

#[test]
fn the_sequences_example_compiles_and_runs() {
    plant_example("sequences");
}

#[test]
fn the_wastewater_station_example_compiles_and_runs() {
    plant_example("wastewater_station");
}

#[test]
fn the_hmi_equipment_example_compiles_and_runs_with_the_plant_library() {
    let folder = repository().join("examples/hmi_equipment/src");
    compile_and_run(
        "examples/hmi_equipment",
        &structured_text_files(&folder),
        &[HMI],
    );
}

#[test]
fn every_complete_example_of_the_plant_manual_compiles_and_runs_with_the_library() {
    let path = repository().join("docs/guides/PLANT_LIBRARY_GUIDE.md");
    let manual = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{}: {error}", path.display()));
    let examples = complete_examples(&manual);
    assert!(
        !examples.is_empty(),
        "the manual shows the blocks with complete examples (specification §13.2): no ```iecst \
         block declares a PROGRAM or FUNCTION_BLOCK"
    );
    for (index, example) in examples.into_iter().enumerate() {
        let heading = example
            .lines()
            .map(str::trim)
            .find(|line| !line.is_empty())
            .unwrap_or_default()
            .to_owned();
        let mut files = vec![example];
        if !declares(&files[0], &["PROGRAM", "CONFIGURATION"]) {
            files.push(STUB_PROGRAM.to_owned());
        }
        compile_and_run(
            &format!("manual example {} ({heading})", index + 1),
            &files,
            &[HMI],
        );
    }
}

#[test]
fn the_shared_sources_include_every_plant_source_file() {
    let folder = repository().join("libraries/plant/src");
    let mut on_disk: Vec<String> = std::fs::read_dir(&folder)
        .unwrap_or_else(|error| panic!("{}: {error}", folder.display()))
        .map(|entry| {
            entry
                .expect("a folder entry")
                .file_name()
                .to_string_lossy()
                .into_owned()
        })
        .filter(|name| name.ends_with(".st"))
        .collect();
    on_disk.sort();
    let mut listed: Vec<String> = PLANT_SOURCE_FILES
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    listed.sort();
    assert_eq!(
        listed, on_disk,
        "plant_support lists every libraries/plant/src/*.st file"
    );
    assert_eq!(
        SOURCES.len(),
        PLANT_SOURCE_FILES.len() + 2,
        "the Plant sources follow the Alarm and Control libraries"
    );
}
