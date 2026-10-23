//! Spec 34 §4.3 HOST-01: the legacy runtime, the legacy VM dispatcher and the
//! legacy harness stay test-only, and the test-only VM adapter runs the core
//! instruction loop instead of a second one.

use std::path::{Path, PathBuf};

include!("../../../tests/support/repository_source_oracle.rs");

fn repository_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repository root")
        .to_path_buf()
}

fn read_repository_source(relative: &str) -> String {
    let root = repository_root();
    repository_source_tree_read_to_string!(
        (root.join(relative), &root),
        roots = ["crates/trust-runtime/src", "crates/trust-runtime-core/src"],
        extension = "rs",
    )
    .unwrap_or_else(|err| panic!("read {relative}: {err}"))
    .replace("\r\n", "\n")
}

fn rust_files_under(dir: &Path, files: &mut Vec<PathBuf>) {
    for entry in
        std::fs::read_dir(dir).unwrap_or_else(|err| panic!("read dir {}: {err}", dir.display()))
    {
        let path = entry.expect("directory entry").path();
        if path.is_dir() {
            rust_files_under(&path, files);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            files.push(path);
        }
    }
}

/// One declaration line with the attribute lines written directly above it.
struct Item {
    text: String,
    attributes: Vec<String>,
}

fn items_with_attributes(source: &str) -> Vec<Item> {
    let mut items = Vec::new();
    let mut attributes = Vec::new();
    for line in source.lines().map(str::trim) {
        if line.is_empty() || line.starts_with("//") {
            continue;
        }
        if line.starts_with("#[") {
            attributes.push(line.to_owned());
        } else {
            items.push(Item {
                text: line.to_owned(),
                attributes: std::mem::take(&mut attributes),
            });
        }
    }
    items
}

fn is_test_only(item: &Item) -> bool {
    item.attributes
        .iter()
        .any(|attribute| attribute == "#[cfg(test)]")
}

fn declared_module(text: &str) -> Option<&str> {
    text.trim_start_matches("pub(crate) ")
        .trim_start_matches("pub ")
        .strip_prefix("mod ")?
        .strip_suffix(';')
        .map(str::trim)
}

fn is_legacy_runtime_module(name: &str) -> bool {
    matches!(name, "core" | "cycle" | "vm") || name.starts_with("legacy_")
}

/// Spec 34 §4.3 HOST-01 (line 266): the legacy runtime, `vm/` dispatcher and `legacy_*` modules are `#[cfg(test)]` only.
#[test]
fn legacy_runtime_modules_are_declared_only_under_cfg_test() {
    let source = read_repository_source("crates/trust-runtime/src/runtime/mod.rs");
    let items = items_with_attributes(&source);

    let mut legacy_modules = Vec::new();
    for item in &items {
        let Some(name) = declared_module(&item.text) else {
            continue;
        };
        if is_legacy_runtime_module(name) {
            legacy_modules.push(name.to_owned());
            assert!(
                is_test_only(item),
                "runtime/mod.rs declares `mod {name};` outside `#[cfg(test)]`; the legacy runtime must stay test-only (spec 34 §4.3 HOST-01)"
            );
        }
    }
    for expected in [
        "core",
        "cycle",
        "vm",
        "legacy_bytecode",
        "legacy_harness",
        "legacy_restart",
        "legacy_retain_snapshot",
        "legacy_retain_store",
    ] {
        assert!(
            legacy_modules.iter().any(|name| name == expected),
            "runtime/mod.rs no longer declares `mod {expected};`; update this gate together with the legacy layout"
        );
    }
}

/// Spec 34 §4.3 HOST-01 (line 266): the legacy fixture re-exports stay `#[cfg(test)]` only.
#[test]
fn legacy_fixture_reexports_are_declared_only_under_cfg_test() {
    let source = read_repository_source("crates/trust-runtime/src/runtime/mod.rs");
    let items = items_with_attributes(&source);
    let reexports: Vec<&Item> = items
        .iter()
        .filter(|item| {
            item.text.contains("LegacyRuntimeFixture") || item.text.contains("LegacyTestHarness")
        })
        .collect();

    assert_eq!(
        reexports.len(),
        2,
        "runtime/mod.rs must re-export exactly LegacyRuntimeFixture and LegacyTestHarness"
    );
    for item in reexports {
        assert!(
            is_test_only(item),
            "legacy re-export `{}` must be `#[cfg(test)]` (spec 34 §4.3 HOST-01)",
            item.text
        );
    }
}

/// Spec 34 §4.3 HOST-01 (line 266): the historical harness fixture builders stay `#[cfg(test)]` only.
#[test]
fn legacy_harness_fixture_builders_are_declared_only_under_cfg_test() {
    for file in [
        "crates/trust-runtime/src/host/harness/api.rs",
        "crates/trust-runtime/src/host/harness/build.rs",
    ] {
        let source = read_repository_source(file);
        let builders: Vec<Item> = items_with_attributes(&source)
            .into_iter()
            .filter(|item| item.text.contains("fn build_legacy_fixture("))
            .collect();

        assert_eq!(
            builders.len(),
            1,
            "{file} must declare exactly one build_legacy_fixture"
        );
        assert!(
            is_test_only(&builders[0]),
            "{file}: `{}` must be `#[cfg(test)]` (spec 34 §4.3 HOST-01)",
            builders[0].text
        );
    }
}

/// Spec 34 §4.3 HOST-01 (line 266): the test-only VM adapter composes the core instruction loop and defines no second loop.
#[test]
fn test_vm_adapter_runs_the_core_instruction_loop() {
    let adapter = read_repository_source("crates/trust-runtime/src/runtime/vm/dispatch.rs");
    assert!(
        adapter.lines().any(|line| line.trim()
            == "pub(super) use trust_runtime_core::vm::hosted::dispatch::execute_pou_stack_with_locals;"),
        "runtime/vm/dispatch.rs must re-export the core execute_pou_stack_with_locals"
    );

    let core_loop = read_repository_source("crates/trust-runtime-core/src/vm/dispatch/entry.rs");
    assert!(
        core_loop.contains("pub fn execute_pou_stack_with_locals("),
        "the instruction loop must live in trust-runtime-core vm/dispatch/entry.rs"
    );

    let root = repository_root();
    let mut files = Vec::new();
    rust_files_under(
        &root.join("crates/trust-runtime/src/runtime/vm"),
        &mut files,
    );
    assert!(
        !files.is_empty(),
        "runtime/vm sources must exist for this gate"
    );
    for file in files {
        let text = std::fs::read_to_string(&file)
            .unwrap_or_else(|err| panic!("read {}: {err}", file.display()));
        assert!(
            !text.contains("fn execute_pou_stack_with_locals("),
            "{} defines a second instruction loop; the legacy VM must reuse the core loop (spec 34 §4.3 HOST-01)",
            file.display()
        );
    }
}
