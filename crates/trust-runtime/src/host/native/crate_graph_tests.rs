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

#[cfg(unix)]
#[test]
fn a_project_named_through_a_link_is_checked_all_the_same() {
    // Catches a check that compares the folders as named with cargo's canonical paths: macOS's
    // temporary folder `/var/folders` is a link to `/private/var/folders`, so on macOS a crate the
    // project builds with went unchecked and its `unsafe` code through (pull request #530's macOS
    // run). Here the project is named through a link while the metadata names its real folders.
    let real = trust_test_support::unique_temp_path("rust-pou-graph-real");
    let crate_folder = real.join("project/rust");
    let dirty = crate_folder.join("vendor/dirty/src");
    std::fs::create_dir_all(crate_folder.join("src")).expect("the crate's folder");
    std::fs::create_dir_all(&dirty).expect("the vendored crate's folder");
    std::fs::write(crate_folder.join("src/lib.rs"), ROOT).expect("the root");
    std::fs::write(
        dirty.join("lib.rs"),
        "pub fn f(p: *const u8) -> u8 { unsafe { *p } }\n",
    )
    .expect("the vendored crate");
    let link = trust_test_support::unique_temp_path("rust-pou-graph-link");
    std::os::unix::fs::symlink(&real, &link).expect("the link");
    let canonical = std::fs::canonicalize(&crate_folder).expect("the real folder");
    let manifest = |folder: &PathBuf| folder.join("Cargo.toml").display().to_string();
    let vendored = canonical.join("vendor/dirty");
    let metadata = json!({
        "packages": [
            package("project", &manifest(&canonical), &["lib"], &canonical.join("src").display().to_string()),
            package("dirty", &manifest(&vendored), &["lib"], &vendored.join("src").display().to_string()),
        ],
        "resolve": { "nodes": [
            { "id": "project", "deps": [{ "pkg": "dirty", "dep_kinds": [{ "kind": null }] }] },
        ]},
    });
    let found = check(&metadata, &link.join("sdk"), &link.join("project/rust"));
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("dirty 1.0.0 holds `unsafe` code at "),
        "{}",
        found[0]
    );
}

#[test]
fn a_crate_outside_the_sdk_depends_only_on_the_crates_the_sdk_publishes() {
    // RU-20: the crates the SDK is built from serve the generated code (`trust-plc-abi`'s `run`
    // trusts the context it is handed), so a crate outside the SDK, the project's or one it builds
    // with, depends on `trust-plc` and `trust-plc-macros` but not on them; what the SDK itself is
    // built from (`libm`) and a test-only dependency stay allowed. Catches a check that lets the
    // project or a vendored crate reach the interface crate, or that refuses the SDK's own edges.
    let root = trust_test_support::unique_temp_path("rust-pou-graph-sdk");
    let sdk = root.join("sdk");
    let crate_folder = root.join("project/rust");
    std::fs::create_dir_all(crate_folder.join("src")).expect("the crate's folder");
    std::fs::write(crate_folder.join("src/lib.rs"), ROOT).expect("the root");
    let manifest = |folder: &PathBuf| folder.join("Cargo.toml").display().to_string();
    let sources = |folder: &PathBuf| folder.join("src").display().to_string();
    let sdk_crate = |name: &str| {
        package(
            name,
            &manifest(&sdk.join(name)),
            &["lib"],
            &sources(&sdk.join(name)),
        )
    };
    let helper = root.join("registry/helper");
    let metadata = json!({
        "packages": [
            package("project", &manifest(&crate_folder), &["lib"], &sources(&crate_folder)),
            sdk_crate("trust-plc"),
            sdk_crate("trust-plc-macros"),
            sdk_crate("trust-plc-abi"),
            sdk_crate("trust-plc-lib"),
            package("libm", "/registry/libm/Cargo.toml", &["lib"], "/registry/libm/src"),
            package("helper", &manifest(&helper), &["lib"], &sources(&helper)),
        ],
        "resolve": { "nodes": [
            { "id": "project", "deps": [
                { "pkg": "trust-plc", "dep_kinds": [{ "kind": null }] },
                { "pkg": "trust-plc-macros", "dep_kinds": [{ "kind": null }] },
                { "pkg": "trust-plc-abi", "dep_kinds": [{ "kind": null }] },
                { "pkg": "trust-plc-lib", "dep_kinds": [{ "kind": "dev" }] },
                { "pkg": "libm", "dep_kinds": [{ "kind": null }] },
                { "pkg": "helper", "dep_kinds": [{ "kind": null }] },
            ]},
            { "id": "helper", "deps": [{ "pkg": "trust-plc-lib", "dep_kinds": [{ "kind": null }] }] },
            { "id": "trust-plc", "deps": [
                { "pkg": "trust-plc-abi", "dep_kinds": [{ "kind": null }] },
                { "pkg": "trust-plc-lib", "dep_kinds": [{ "kind": null }] },
                { "pkg": "trust-plc-macros", "dep_kinds": [{ "kind": null }] },
            ]},
            { "id": "trust-plc-lib", "deps": [{ "pkg": "libm", "dep_kinds": [{ "kind": null }] }] },
        ]},
    });
    let found = check(&metadata, &sdk, &crate_folder);
    let expected = [
        "helper 1.0.0 depends on trust-plc-lib 1.0.0, which the SDK is built from",
        "project 1.0.0 depends on trust-plc-abi 1.0.0, which the SDK is built from",
    ];
    assert_eq!(found.len(), expected.len(), "{found:#?}");
    for (message, start) in found.iter().zip(expected) {
        assert!(message.starts_with(start), "{message}\n  expected: {start}");
        assert!(message.contains("RU-20"), "{message}");
    }
}

#[test]
fn the_generated_package_in_a_rust_first_project_is_the_builds_own() {
    // RU-20, RU-65: in a Rust-first project the crate's folder is the project's own, so the build's
    // generated package, which `cargo metadata` names as its root, lies inside it; it depends on the
    // crates the SDK is built from as the generated code does, while the project's crate still may
    // not. Catches the generated package taken for one of the project's crates, which refused every
    // Rust-first project.
    let root = trust_test_support::unique_temp_path("rust-pou-graph-first");
    let sdk = root.join("sdk");
    let crate_folder = root.join("project");
    let generated = crate_folder.join("build/native/x86_64-unknown-linux-gnu/module");
    std::fs::create_dir_all(crate_folder.join("src")).expect("the crate's folder");
    std::fs::write(crate_folder.join("src/lib.rs"), ROOT).expect("the root");
    std::fs::create_dir_all(generated.join("src")).expect("the generated package's folder");
    let manifest = |folder: &PathBuf| folder.join("Cargo.toml").display().to_string();
    let sources = |folder: &PathBuf| folder.join("src").display().to_string();
    let sdk_crate = |name: &str| {
        package(
            name,
            &manifest(&sdk.join(name)),
            &["lib"],
            &sources(&sdk.join(name)),
        )
    };
    let metadata = |project_reaches_abi: bool| {
        let mut project_deps = vec![json!({ "pkg": "trust-plc", "dep_kinds": [{ "kind": null }] })];
        if project_reaches_abi {
            project_deps.push(json!({ "pkg": "trust-plc-abi", "dep_kinds": [{ "kind": null }] }));
        }
        json!({
            "packages": [
                package("trust_module", &manifest(&generated), &["lib"], &sources(&generated)),
                package("project", &manifest(&crate_folder), &["lib"], &sources(&crate_folder)),
                sdk_crate("trust-plc"),
                sdk_crate("trust-plc-abi"),
                sdk_crate("trust-plc-lib"),
            ],
            "resolve": {
                "root": "trust_module",
                "nodes": [
                    { "id": "trust_module", "deps": [
                        { "pkg": "project", "dep_kinds": [{ "kind": null }] },
                        { "pkg": "trust-plc-abi", "dep_kinds": [{ "kind": null }] },
                        { "pkg": "trust-plc-lib", "dep_kinds": [{ "kind": null }] },
                    ]},
                    { "id": "project", "deps": project_deps },
                    { "id": "trust-plc", "deps": [
                        { "pkg": "trust-plc-abi", "dep_kinds": [{ "kind": null }] },
                    ]},
                ],
            },
        })
    };
    assert_eq!(
        check(&metadata(false), &sdk, &crate_folder),
        Vec::<String>::new()
    );
    let found = check(&metadata(true), &sdk, &crate_folder);
    assert_eq!(found.len(), 1, "{found:#?}");
    assert!(
        found[0].starts_with("project 1.0.0 depends on trust-plc-abi 1.0.0"),
        "{found:#?}"
    );
}
