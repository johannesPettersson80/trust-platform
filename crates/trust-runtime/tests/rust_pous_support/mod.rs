//! What the tests of projects with Rust POUs share (specification 48): a project folder with its
//! crate and its declarations, its Structured Text as `trust-runtime build` collects it, and the
//! project built for this computer with the toolchain of specification 44 §4.6 at
//! `TRUST_TOOLCHAIN`.
#![allow(dead_code)]

use std::path::{Path, PathBuf};

use trust_runtime::harness::SourceFile;
use trust_runtime::native::rust_pous::{layout, write_declarations, Written};
use trust_runtime::native::{build_project, Build, Device, Options, Target, Toolchain};
use trust_runtime::stdlib::conversions::ConversionProfile;

/// The project's Structured Text, as `trust-runtime build` collects it: its program, when it has
/// one, and the generated declarations and configuration where its layout puts them.
pub fn sources(root: &Path) -> Vec<SourceFile> {
    let layout = layout(root).expect("the project's layout");
    std::iter::once("src/main.st".to_string())
        .chain(
            [
                Some(layout.declarations),
                layout.configuration,
                Some(layout.services),
            ]
            .into_iter()
            .flatten(),
        )
        .filter(|label| root.join(label).is_file())
        .map(|label| {
            let text = std::fs::read_to_string(root.join(&label)).expect("the source");
            SourceFile::with_path(&label, text)
        })
        .collect()
}

/// The project built for this computer; `None` with a toolchain that has no cargo and SDK sources,
/// whose refusal names them.
pub fn built(root: &Path) -> Option<(Build, Device)> {
    built_under(root, ConversionProfile::Iec)
}

/// The project built for this computer under the profile `conversions`, as [`built`] builds it
/// under the IEC profile.
pub fn built_under(root: &Path, conversions: ConversionProfile) -> Option<(Build, Device)> {
    built_from(root, &sources(root), conversions)
}

/// Every Structured Text file below the project's `src/`, as `trust-runtime build --sources src`
/// collects them, in byte order.
pub fn project_sources(root: &Path) -> Vec<SourceFile> {
    fn walk(root: &Path, folder: &Path, found: &mut Vec<SourceFile>) {
        let mut entries: Vec<PathBuf> = std::fs::read_dir(folder)
            .expect("the folder reads")
            .map(|entry| entry.expect("an entry").path())
            .collect();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                walk(root, &path, found);
            } else if path.extension().is_some_and(|extension| extension == "st") {
                let label = path.strip_prefix(root).expect("below the root");
                let text = std::fs::read_to_string(&path).expect("the source");
                found.push(SourceFile::with_path(
                    label.to_string_lossy().replace('\\', "/"),
                    text,
                ));
            }
        }
    }
    let mut found = Vec::new();
    walk(root, &root.join("src"), &mut found);
    found
}

/// The project built for this computer from `sources` under the profile `conversions`; `None`
/// with a toolchain that has no cargo and SDK sources, whose refusal names them.
pub fn built_from(
    root: &Path,
    sources: &[SourceFile],
    conversions: ConversionProfile,
) -> Option<(Build, Device)> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let target = Target::this_cpu().expect("a computer target");
    let work = root.join("build/native");
    let result = build_project(
        root,
        sources,
        &[],
        &Options {
            target,
            hooks: true,
        },
        conversions,
        &toolchain,
        &work,
    );
    if toolchain.cargo.is_none() || toolchain.sdk.is_none() {
        let error = result.err().expect("the build is refused").to_string();
        assert!(error.contains("no cargo and SDK sources"), "{error}");
        return None;
    }
    let built = result.unwrap_or_else(|error| panic!("the project builds: {error}"));
    let device = Device {
        target,
        program_memory: 1 << 20,
        task_stack: 1 << 20,
        max_statements: 1_000,
        trusted_keys: &[],
    };
    Some((built, device))
}

/// The error of building the project in `root` for this computer, with the toolchain.
pub fn build_error(root: &Path) -> (String, Toolchain) {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let error = build_project(
        root,
        &sources(root),
        &[],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        ConversionProfile::Iec,
        &toolchain,
        &root.join("build/native"),
    )
    .err()
    .expect("the build is refused")
    .to_string();
    (error, toolchain)
}

/// A project with the crate files `files` (relative to `rust/`) and the program `program`.
pub fn custom_project(name: &str, files: &[(&str, &str)], program: &str) -> PathBuf {
    let root = trust_test_support::unique_temp_path(name);
    for (path, text) in files {
        let file = root.join("rust").join(path);
        std::fs::create_dir_all(file.parent().expect("a folder")).expect("the folder");
        std::fs::write(file, text).expect("the file");
    }
    std::fs::create_dir_all(root.join("src")).expect("the sources' folder");
    std::fs::write(root.join("src/main.st"), program).expect("the program");
    let written = write_declarations(&root);
    assert!(matches!(written, Ok(Written::Wrote(_))), "{written:?}");
    root
}

/// The crate's manifest with the dependencies `extra`.
pub fn manifest(extra: &str) -> String {
    format!(
        "[package]\nname = \"rules\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n\
         [dependencies]\ntrust-plc = \"={}\"\n{extra}",
        env!("CARGO_PKG_VERSION")
    )
}
