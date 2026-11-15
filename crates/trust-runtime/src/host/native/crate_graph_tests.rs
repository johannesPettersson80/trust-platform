//! The scan crate graph keeps specification 48's rules (RU-09, RU-20); the graph is cargo's metadata
//! as `cargo metadata` gives it, here written by hand.

use std::path::PathBuf;

use serde_json::json;

use super::check;

const ROOT: &str = "#![cfg_attr(not(test), no_std)]\n#![forbid(unsafe_code)]\n";

/// A package of a metadata graph.
fn package(id: &str, manifest: &str, kinds: &[&str], sources: &str) -> serde_json::Value {
    json!({
        "id": id,
        "name": id,
        "version": "1.0.0",
        "manifest_path": manifest,
        "targets": [
            { "kind": ["lib"], "src_path": format!("{sources}/lib.rs") },
            { "kind": kinds, "src_path": format!("{sources}/build.rs") },
        ],
    })
}

#[test]
fn the_graph_refuses_build_scripts_procedural_macros_and_unsafe_outside_the_sdk() {
    // RU-09 and RU-20: the SDK and the crates it is built from may have build scripts, procedural
    // macros and unsafe code; a crate the project builds with may have none. Catches a check that
    // lets one through, or refuses the SDK's own (proc-macro2's build script, the macros' crate).
    let root = trust_test_support::unique_temp_path("rust-pou-graph");
    let sdk = root.join("sdk");
    let crate_folder = root.join("project/rust");
    let write = |path: PathBuf, text: &str| {
        std::fs::create_dir_all(path.parent().expect("a folder")).expect("the folder");
        std::fs::write(path, text).expect("the file");
    };
    let clean = root.join("registry/clean/src");
    let dirty = root.join("registry/dirty/src");
    write(
        clean.join("lib.rs"),
        "// unsafe in a comment\npub fn f() -> &'static str { \"unsafe\" }\n",
    );
    write(dirty.join("lib.rs"), "pub mod inner;\n");
    write(
        dirty.join("inner.rs"),
        "pub fn read(p: *const u8) -> u8 {\n    unsafe { *p }\n}\n",
    );
    write(crate_folder.join("src/lib.rs"), ROOT);
    let path = |folder: &PathBuf| folder.join("Cargo.toml").display().to_string();
    let metadata = json!({
        "packages": [
            package("project", &path(&crate_folder), &["lib"], &crate_folder.join("src").display().to_string()),
            package("trust-plc", &path(&sdk.join("trust-plc")), &["lib"], &sdk.join("trust-plc/src").display().to_string()),
            package("trust-plc-macros", &path(&sdk.join("trust-plc-macros")), &["proc-macro"], &sdk.join("trust-plc-macros/src").display().to_string()),
            package("proc-macro2", "/registry/proc-macro2/Cargo.toml", &["custom-build"], "/registry/proc-macro2/src"),
            package("clean", &path(&root.join("registry/clean")), &["lib"], &clean.display().to_string()),
            package("dirty", &path(&root.join("registry/dirty")), &["lib"], &dirty.display().to_string()),
            package("scripted", "/registry/scripted/Cargo.toml", &["custom-build"], "/registry/scripted/src"),
            package("derive", "/registry/derive/Cargo.toml", &["proc-macro"], "/registry/derive/src"),
            package("tester", "/registry/tester/Cargo.toml", &["custom-build"], "/registry/tester/src"),
        ],
        "resolve": { "nodes": [
            { "id": "project", "deps": [
                { "pkg": "trust-plc", "dep_kinds": [{ "kind": null }] },
                { "pkg": "clean", "dep_kinds": [{ "kind": null }] },
                { "pkg": "dirty", "dep_kinds": [{ "kind": null }] },
                { "pkg": "scripted", "dep_kinds": [{ "kind": null }] },
                { "pkg": "derive", "dep_kinds": [{ "kind": null }] },
                { "pkg": "tester", "dep_kinds": [{ "kind": "dev" }] },
            ]},
            { "id": "trust-plc", "deps": [{ "pkg": "trust-plc-macros", "dep_kinds": [{ "kind": null }] }] },
            { "id": "trust-plc-macros", "deps": [{ "pkg": "proc-macro2", "dep_kinds": [{ "kind": null }] }] },
        ]},
    });
    let found = check(&metadata, &sdk, &crate_folder);
    let expected = [
        "derive 1.0.0 is a procedural macro",
        "dirty 1.0.0 holds `unsafe` code at ",
        "scripted 1.0.0 has a build script",
    ];
    assert_eq!(found.len(), expected.len(), "{found:#?}");
    for (message, start) in found.iter().zip(expected) {
        assert!(message.starts_with(start), "{message}\n  expected: {start}");
    }
    assert!(found[1].contains("inner.rs:2"), "{}", found[1]);
}
