//! The build holds a project's crate to specification 48's rules for its manifest and root (RU-08);
//! every expected refusal is the specification's.

use std::path::PathBuf;

use super::check_crate;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// A project whose crate has the manifest `manifest` and the root `root`.
fn project(manifest: &str, root: &str) -> PathBuf {
    let folder = trust_test_support::unique_temp_path("rust-pou-rules");
    std::fs::create_dir_all(folder.join("rust/src")).expect("the crate's folder");
    std::fs::write(folder.join("rust/Cargo.toml"), manifest).expect("the manifest");
    std::fs::write(folder.join("rust/src/lib.rs"), root).expect("the root");
    folder
}

fn manifest(extra: &str) -> String {
    format!(
        "[package]\nname = \"line\"\nversion = \"0.1.0\"\nedition = \"2024\"\n{extra}\n\
         [dependencies]\ntrust-plc = \"={VERSION}\"\n"
    )
}

const ROOT: &str = "#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n";

fn refusals(manifest: &str, root: &str) -> Vec<String> {
    let folder = project(manifest, root);
    match check_crate(&folder, &folder.join("rust/src/lib.rs")) {
        Ok(()) => Vec::new(),
        Err(errors) => errors.iter().map(ToString::to_string).collect(),
    }
}

#[test]
fn a_crate_that_keeps_the_rules_builds() {
    // RU-08's crate: an rlib of edition 2024, the SDK at this release's version, a root without
    // the standard library and with unsafe code forbidden; `#![no_std]` keeps the rule too, and
    // the SDK may be named in a table.
    assert_eq!(refusals(&manifest(""), ROOT), Vec::<String>::new());
    assert_eq!(
        refusals(
            &manifest(""),
            "#![no_std]\n#![forbid(unsafe_code, missing_docs)]\n"
        ),
        Vec::<String>::new()
    );
    let table = manifest("").replace(
        &format!("trust-plc = \"={VERSION}\""),
        &format!("trust-plc = {{ version = \"={VERSION}\" }}"),
    );
    assert_eq!(refusals(&table, ROOT), Vec::<String>::new());
}

#[test]
fn each_rule_a_crate_breaks_is_refused_with_the_line_to_write() {
    // Catches a build that compiles a crate RU-08 refuses: each case breaks one rule, and the
    // refusal names the line that keeps it.
    let cases: [(String, &str, &str); 7] = [
        (
            manifest("").replace("2024", "2021"),
            ROOT,
            "edition = \"2024\"",
        ),
        (
            format!(
                "{}\n[[bin]]\nname = \"tool\"\npath = \"src/main.rs\"\n",
                manifest("")
            ),
            ROOT,
            "without a [[bin]]",
        ),
        (
            manifest("\n[lib]\ncrate-type = [\"cdylib\"]\n"),
            ROOT,
            "is an rlib",
        ),
        (
            manifest("").replace(&format!("\"={VERSION}\""), "\"0.1\""),
            ROOT,
            "trust-plc = \"=",
        ),
        (
            manifest(""),
            "#![forbid(unsafe_code)]\n",
            "#![cfg_attr(not(test), no_std)]",
        ),
        (
            manifest(""),
            "#![cfg_attr(not(test), no_std)]\n",
            "#![forbid(unsafe_code)]",
        ),
        (
            manifest(""),
            "#![cfg_attr(not(test), no_std)]\n#![deny(unsafe_code)]\n",
            "#![forbid(unsafe_code)]",
        ),
    ];
    for (manifest, root, expected) in cases {
        let found = refusals(&manifest, root);
        assert_eq!(found.len(), 1, "{expected}: {found:?}");
        assert!(found[0].contains(expected), "{expected}: {found:?}");
        assert!(found[0].contains("RU-08"), "{found:?}");
    }
}
