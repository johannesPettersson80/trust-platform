use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::{extract_from_repository, unit_context};

static FIXTURE_COUNTER: AtomicUsize = AtomicUsize::new(0);

fn unit_context_fixture_root() -> PathBuf {
    let ordinal = FIXTURE_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!(
        "trust-test-assertions-unit-context-{}-{ordinal}",
        std::process::id()
    ));
    if root.exists() {
        fs::remove_dir_all(&root).expect("remove stale unit-context fixture");
    }
    fs::create_dir_all(root.join("crates/example/src/runtime/tests"))
        .expect("create unit-context fixture");
    root
}

#[test]
fn repository_unit_context_follows_test_module_sibling_helpers() {
    let root = unit_context_fixture_root();
    fs::write(
        root.join("crates/example/Cargo.toml"),
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    fs::write(root.join("crates/example/src/lib.rs"), "mod runtime;\n")
        .expect("write fixture crate root");
    fs::write(
        root.join("crates/example/src/runtime.rs"),
        "#[cfg(test)]\nmod tests;\n",
    )
    .expect("write fixture parent module");
    fs::write(
        root.join("crates/example/src/runtime/tests.rs"),
        "mod cases;\nmod support;\n",
    )
    .expect("write fixture test module");
    fs::write(
        root.join("crates/example/src/runtime/tests/support.rs"),
        "pub(super) fn exercise_contract() { assert!(true); }\n",
    )
    .expect("write fixture helper module");
    let test_path = "crates/example/src/runtime/tests/cases.rs";
    fs::write(
        root.join(test_path),
        concat!(
            "use super::support::exercise_contract;\n",
            "#[test]\n",
            "fn delegates() { exercise_contract(); }\n",
        ),
    )
    .expect("write fixture test module");

    let inventory = extract_from_repository(&root, test_path, "delegates", "rust_unit_test")
        .expect("extract repository unit-test assertions");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "unit context must resolve sibling helper: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert_eq!(1, inventory.assertions.len());
    assert_eq!(
        "runtime::tests::support::exercise_contract",
        inventory.assertions[0].function
    );
    assert_eq!(
        "crates/example/src/runtime/tests/support.rs",
        inventory.assertions[0].source_path
    );
    fs::remove_dir_all(root).expect("remove unit-context fixture");
}

#[test]
fn repository_unit_context_selects_the_target_that_owns_the_test_module() {
    let root = unit_context_fixture_root();
    fs::write(
        root.join("crates/example/Cargo.toml"),
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    fs::write(
        root.join("crates/example/src/lib.rs"),
        "pub mod document_text;\n",
    )
    .expect("write fixture library root");
    fs::write(
        root.join("crates/example/src/document_text.rs"),
        "pub fn normalize() {}\n",
    )
    .expect("write fixture library module");
    fs::write(root.join("crates/example/src/main.rs"), "mod runtime;\n")
        .expect("write fixture binary root");
    fs::write(
        root.join("crates/example/src/runtime.rs"),
        "#[cfg(test)]\nmod tests;\n",
    )
    .expect("write fixture binary module");
    let test_path = "crates/example/src/runtime/tests.rs";
    fs::write(
        root.join(test_path),
        "#[test]\nfn binary_contract() { assert!(true); }\n",
    )
    .expect("write fixture binary test module");

    assert_eq!(
        root.join("crates/example/src/main.rs"),
        unit_context::target_root(&root, PathBuf::from(test_path).as_path())
            .expect("resolve owning target root")
    );
    let inventory = extract_from_repository(&root, test_path, "binary_contract", "rust_unit_test")
        .expect("select the binary target that owns the unit-test module");

    assert_eq!(1, inventory.assertions.len());
    assert_eq!(
        "runtime::tests::binary_contract",
        inventory.assertions[0].function
    );
    fs::remove_dir_all(root).expect("remove unit-context fixture");
}

#[test]
fn repository_unit_context_does_not_treat_production_calls_as_test_oracles() {
    let root = unit_context_fixture_root();
    fs::write(
        root.join("crates/example/Cargo.toml"),
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    fs::write(root.join("crates/example/src/lib.rs"), "mod runtime;\n")
        .expect("write fixture crate root");
    fs::write(
        root.join("crates/example/src/runtime.rs"),
        concat!(
            "use std::path::Path;\n",
            "fn run_runtime(path: &Path) { let _ = std::fs::read_to_string(path).unwrap(); }\n",
            "#[cfg(test)]\n",
            "mod tests;\n",
        ),
    )
    .expect("write fixture production module");
    let test_path = "crates/example/src/runtime/tests.rs";
    fs::write(
        root.join(test_path),
        concat!(
            "use super::run_runtime;\n",
            "#[test]\n",
            "fn delegates() { run_runtime(std::path::Path::new(\"generated.txt\")); }\n",
        ),
    )
    .expect("write fixture test module");

    let inventory = extract_from_repository(&root, test_path, "delegates", "rust_unit_test")
        .expect("extract unit-test assertions without traversing product execution");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "production runtime observations are not delegated test oracles: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert!(
        inventory.assertions.is_empty(),
        "production unwraps are not delegated test assertions: {:?}",
        inventory.assertions
    );
    fs::remove_dir_all(root).expect("remove unit-context fixture");
}

#[test]
fn repository_unit_context_does_not_follow_inline_tests_into_production_functions() {
    let root = unit_context_fixture_root();
    fs::write(
        root.join("crates/example/Cargo.toml"),
        "[package]\nname = \"example\"\nversion = \"0.1.0\"\nedition = \"2021\"\n",
    )
    .expect("write fixture manifest");
    fs::write(root.join("crates/example/src/lib.rs"), "mod runtime;\n")
        .expect("write fixture crate root");
    let test_path = "crates/example/src/runtime.rs";
    fs::write(
        root.join(test_path),
        concat!(
            "use std::path::Path;\n",
            "fn run_runtime(path: &Path) { let _ = std::fs::read_to_string(path).unwrap(); }\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use super::run_runtime;\n",
            "    #[test]\n",
            "    fn delegates() { run_runtime(std::path::Path::new(\"generated.txt\")); }\n",
            "}\n",
        ),
    )
    .expect("write fixture module with inline tests");

    let inventory = extract_from_repository(&root, test_path, "delegates", "rust_unit_test")
        .expect("extract inline unit-test assertions without traversing product execution");

    assert!(
        inventory.unresolved_assertion_helpers.is_empty(),
        "same-source production observations are not delegated test oracles: {:?}",
        inventory.unresolved_assertion_helpers
    );
    assert!(
        inventory.assertions.is_empty(),
        "same-source production unwraps are not delegated test assertions: {:?}",
        inventory.assertions
    );
    fs::remove_dir_all(root).expect("remove unit-context fixture");
}
