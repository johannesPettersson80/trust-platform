//! The manifest of the build's package (specification 48 §9.3): the paths that cargo reads in it.

use super::*;

#[test]
fn a_canonical_windows_path_is_written_as_cargo_reads_it() {
    // A canonical folder on Windows carries the verbatim prefix `\\?\`, which cargo refuses in a
    // manifest as an "invalid path url": a machine test of a Rust-first project failed so on
    // Windows (26 November 2026). Catches a manifest path that keeps the prefix, of a drive's
    // folder or of a network share's, and one that keeps `\`.
    assert_eq!(cargo_path(Path::new(r"\\?\D:\a\cell")), "D:/a/cell");
    assert_eq!(
        cargo_path(Path::new(r"\\?\UNC\server\share\cell")),
        "//server/share/cell"
    );
    assert_eq!(cargo_path(Path::new(r"D:\a\cell")), "D:/a/cell");
    assert_eq!(cargo_path(Path::new("/home/me/cell")), "/home/me/cell");
}

#[test]
fn the_manifest_names_canonical_folders_as_cargo_reads_them() {
    // trust-sim builds a machine test's project at its canonical folder (specification 48 RU-58):
    // the manifest's dependencies on the project's crate, its bindings, the SDK's crates and the
    // vendored crates name their folders without the verbatim prefix. Catches any of the four
    // written as the canonical path gives it.
    let project = ProjectCrate {
        folder: PathBuf::from(r"\\?\D:\a\cell"),
        label: ".".to_owned(),
        package: "rust-first-cell".to_owned(),
        bindings: Some(PathBuf::from(r"\\?\D:\a\cell\build\trust_st")),
        debug: false,
    };
    let vendored = [(
        "libm".to_owned(),
        PathBuf::from(r"\\?\C:\trust\vendor\libm"),
    )];
    let text = manifest(
        Target::X86_64,
        Path::new(r"\\?\C:\trust\sdk"),
        &vendored,
        &project,
    );
    for line in [
        r#"trust_project = { path = "D:/a/cell", package = "rust-first-cell" }"#,
        r#"trust_st = { path = "D:/a/cell/build/trust_st" }"#,
        r#"trust-plc = { path = "C:/trust/sdk/trust-plc" }"#,
        r#"libm = { path = "C:/trust/vendor/libm" }"#,
    ] {
        assert!(text.contains(line), "{line} in\n{text}");
    }
    assert!(!text.contains("?"), "{text}");
}
