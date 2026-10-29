use std::path::{Path, PathBuf};

use trust_runtime::config::{IoConfig, RuntimeConfig};

fn collect_runtime_configs(directory: &Path, configs: &mut Vec<PathBuf>) {
    let entries = std::fs::read_dir(directory)
        .unwrap_or_else(|error| panic!("read {}: {error}", directory.display()));
    for entry in entries {
        let entry = entry.expect("read example directory entry");
        let path = entry.path();
        if path.is_dir() {
            collect_runtime_configs(&path, configs);
        } else if path.file_name().is_some_and(|name| name == "runtime.toml") {
            configs.push(path);
        }
    }
}

#[test]
fn shipped_example_runtime_configs_are_loadable() {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let mut configs = Vec::new();
    collect_runtime_configs(&examples, &mut configs);
    configs.sort();
    assert!(
        !configs.is_empty(),
        "no shipped runtime.toml examples found"
    );

    let failures = configs
        .iter()
        .filter_map(|path| {
            RuntimeConfig::load(path)
                .err()
                .map(|error| format!("{}: {error}", path.display()))
        })
        .collect::<Vec<_>>();
    assert!(
        failures.is_empty(),
        "invalid shipped runtime.toml examples:\n{}",
        failures.join("\n")
    );
}

#[test]
fn reliability_test_project_configs_are_loadable() {
    // The nightly load and soak tests (scripts/runtime_load_test.sh, scripts/runtime_soak_test.sh)
    // run this project. Its runtime.toml kept an empty `runtime.mesh.auth_token`, which the runtime
    // refuses (specification 11, 6.8.0a), so from 25 August 2026 every nightly run ended before its
    // first sample. Here every pull request refuses such a project.
    let project = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/runtime_reliability_bundle");
    RuntimeConfig::load(project.join("runtime.toml"))
        .unwrap_or_else(|error| panic!("the reliability test project's runtime.toml: {error}"));
    IoConfig::load(project.join("io.toml"))
        .unwrap_or_else(|error| panic!("the reliability test project's io.toml: {error}"));
}
