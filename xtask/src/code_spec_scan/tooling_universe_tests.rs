use std::fs;

use super::tests::fixture_root;
use super::tooling_universe::scan_tooling_rust_universe;

#[test]
fn tooling_universe_scans_xtask_graph_and_fuzz_targets_but_not_cfg_test_modules() {
    let root = fixture_root("tooling-universe");
    fs::create_dir_all(root.join("xtask/src")).unwrap();
    fs::write(
        root.join("xtask/src/main.rs"),
        "mod command;\n#[cfg(test)] mod tests;\nfn main() {}\n",
    )
    .unwrap();
    fs::write(root.join("xtask/src/command.rs"), "pub fn run() {}\n").unwrap();
    fs::write(root.join("xtask/src/tests.rs"), "fn fixture_only() {}\n").unwrap();
    fs::create_dir_all(root.join("fuzz/fuzz_targets")).unwrap();
    fs::write(
        root.join("fuzz/fuzz_targets/parser.rs"),
        "fn fuzz_target(data: &[u8]) { let _ = data; }\n",
    )
    .unwrap();

    let scan = scan_tooling_rust_universe(&root);

    assert!(scan.diagnostics.is_empty(), "{:?}", scan.diagnostics);
    let paths = scan
        .sources
        .iter()
        .map(|source| source.path.as_str())
        .collect::<Vec<_>>();
    assert!(paths.contains(&"xtask/src/main.rs"));
    assert!(paths.contains(&"xtask/src/command.rs"));
    assert!(paths.contains(&"fuzz/fuzz_targets/parser.rs"));
    assert!(!paths.contains(&"xtask/src/tests.rs"));
    fs::remove_dir_all(root).unwrap();
}
