//! The crate files of an isolated run (protocol annex E8.2): what is copied and what is not, the
//! limits, a dependency outside the crate, and the copied cargo configuration read in place.

use std::path::Path;

use base64::Engine as _;

use super::{read, write, CrateFile, MAX_FILES, TOO_LARGE};

fn file(root: &Path, path: &str, bytes: &[u8]) {
    let path = root.join(path);
    std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
    std::fs::write(path, bytes).expect("the file");
}

const MANIFEST: &str = "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
                        [dependencies]\nhelper = { path = \"helper\" }\n";

/// A project with the crate `rust/`, a path crate inside it, its outputs and vendored sources, and
/// the project's `trust.toml` and cargo configuration.
fn project(name: &str) -> std::path::PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    file(&root, "rust/Cargo.toml", MANIFEST.as_bytes());
    file(&root, "rust/Cargo.lock", b"version = 4\n");
    file(&root, "rust/src/lib.rs", b"#![no_std]\n");
    file(&root, "rust/src/table.bin", &[0, 159, 255, 7]);
    file(
        &root,
        "rust/helper/Cargo.toml",
        b"[package]\nname = \"helper\"\nversion = \"0.3.1\"\n",
    );
    file(&root, "rust/helper/src/lib.rs", b"#![no_std]\n");
    file(&root, "rust/target/debug/out.rlib", b"output");
    file(&root, "rust/vendor/heapless/Cargo.toml", b"[package]\n");
    file(
        &root,
        "trust.toml",
        b"[timing]\nprofile = \"development\"\n",
    );
    file(&root, ".cargo/config.toml", b"[source.crates-io]\nreplace-with = \"vendored\"\n\n[source.vendored]\ndirectory = \"vendor\"\n");
    file(&root, "src/main.st", b"PROGRAM Main\nEND_PROGRAM\n");
    root
}

#[test]
fn the_crates_folder_is_copied_without_its_outputs_and_vendored_sources() {
    // E8.2: every file of the crate's folder but its `target`, `build`, `vendor`, `.git` and
    // `node_modules` folders, with the project's `trust.toml` and `.cargo/config.toml`, by their
    // paths in the project, in order; a file that is not text keeps its bytes. Catches a copy
    // without the path crate the build needs, one with the outputs or the vendored sources (read in
    // place), the Structured Text sources (the snapshot's own) or a binary file garbled as text.
    let root = project("crate-files-copied");
    let files = read(&root).expect("the crate's files read");
    let paths: Vec<&str> = files.iter().map(|file| file.path.as_str()).collect();
    assert_eq!(
        paths,
        [
            ".cargo/config.toml",
            "rust/Cargo.lock",
            "rust/Cargo.toml",
            "rust/helper/Cargo.toml",
            "rust/helper/src/lib.rs",
            "rust/src/lib.rs",
            "rust/src/table.bin",
            "trust.toml",
        ]
    );
    let table = files
        .iter()
        .find(|file| file.path == "rust/src/table.bin")
        .expect("the table");
    assert_eq!(
        base64::engine::general_purpose::STANDARD
            .decode(&table.bytes)
            .expect("base64"),
        [0, 159, 255, 7]
    );
}

#[test]
fn a_project_without_a_crate_has_no_crate_files() {
    // E8.2: only a project with a Rust crate adds files. Catches a snapshot that copies the project
    // of a Structured Text program.
    let root = trust_test_support::unique_temp_path("crate-files-none");
    file(&root, "src/main.st", b"PROGRAM Main\nEND_PROGRAM\n");
    assert_eq!(read(&root), Ok(Vec::new()));
}

#[test]
fn a_crate_past_the_limits_is_refused() {
    // E8.2: at most 256 files and 4,194,304 bytes. Catches an unbounded copy into every run.
    let many = project("crate-files-many");
    for index in 0..MAX_FILES {
        file(&many, &format!("rust/src/part{index}.rs"), b"\n");
    }
    assert_eq!(read(&many), Err(TOO_LARGE.to_string()));
    let large = project("crate-files-large");
    file(&large, "rust/src/blob.bin", &vec![1u8; 4 * 1024 * 1024]);
    assert_eq!(read(&large), Err(TOO_LARGE.to_string()));
}

#[test]
fn a_dependency_outside_the_crates_folder_is_refused_with_its_path() {
    // E8.2: a dependency whose `path` leaves the crate's folder is not in the copy, so the project
    // is not ready, naming it; one inside (`helper`) is copied. Catches a run whose build would find
    // the dependency missing, and one refused for a path crate it holds.
    let root = project("crate-files-outside");
    file(
        &root,
        "rust/Cargo.toml",
        format!("{MANIFEST}shared = {{ path = \"../shared\" }}\n").as_bytes(),
    );
    assert_eq!(
        read(&root),
        Err("the Rust crate's dependency ../shared lies outside its folder, which an isolated run does not copy".into())
    );
}

#[test]
fn the_copied_cargo_configuration_reads_the_vendored_sources_in_place() {
    // E8.2: the run's build reads the crate's registry dependencies where the project's cargo
    // configuration puts them; the copy's relative `directory` becomes the project's folder.
    // Catches a run that looks for `vendor/` in its own folder, where it is not copied, and a file
    // written outside the run's folder.
    let root = project("crate-files-config");
    let files = read(&root).expect("the crate's files read");
    let run = trust_test_support::unique_temp_path("crate-files-run");
    write(&files, &run, &root).expect("the files are written");
    let config: toml::Table = toml::from_str(
        &std::fs::read_to_string(run.join(".cargo/config.toml")).expect("the configuration"),
    )
    .expect("the configuration reads");
    assert_eq!(
        config["source"]["vendored"]["directory"].as_str(),
        Some(root.join("vendor").to_string_lossy().as_ref())
    );
    assert_eq!(
        config["source"]["crates-io"]["replace-with"].as_str(),
        Some("vendored")
    );
    assert_eq!(
        std::fs::read(run.join("rust/src/table.bin")).expect("the table"),
        [0, 159, 255, 7]
    );
    let outside = CrateFile {
        path: "../escape.rs".into(),
        bytes: String::new(),
    };
    assert!(write(&[outside], &run, &root).is_err());
}
