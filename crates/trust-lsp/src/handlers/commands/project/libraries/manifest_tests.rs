//! `trust-lsp.toml` edits of the Libraries panel (specification 25 §11.6: dependency edits keep
//! unrelated manifest structure; path and Git dependencies parse deterministically; a Git dependency
//! has exactly one reviewed pin selector; update and removal change only the selected entry). The
//! first six cases are the extension's former `libraries-model.test.ts` assertions.

use super::*;
use crate::config::manifest_dependencies;

fn path_spec(path: &str, version: &str) -> DependencySpec {
    DependencySpec::path(path.to_owned(), Some(version.to_owned()))
}

#[test]
fn adding_a_dependency_keeps_the_manifest_and_appends_the_table() {
    // Catches: a rewrite of the project's own settings, or a table without the blank line before it.
    let updated = upsert(
        "include_paths = [\"src\"]\n",
        "OSCAT",
        &path_spec("libraries/oscat", "0.1.0"),
    )
    .expect("upsert");
    assert_eq!(
        updated,
        "include_paths = [\"src\"]\n\n[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\n"
    );
    assert_eq!(
        upsert("", "OSCAT", &path_spec("libraries/oscat", "0.1.0")).expect("upsert"),
        "[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\n",
        "an empty manifest gets the table without a blank line first"
    );
}

#[test]
fn updating_and_removing_change_only_the_selected_entry() {
    let manifest = "[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.0.1\" }\nOther = { path = \"vendor/other\" }\n";
    let updated =
        upsert(manifest, "OSCAT", &path_spec("libraries/oscat", "0.1.0")).expect("upsert");
    assert_eq!(
        updated,
        "[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\nOther = { path = \"vendor/other\" }\n"
    );
    let removed = remove(&updated, "OSCAT").expect("remove");
    assert_eq!(
        removed,
        "[dependencies]\nOther = { path = \"vendor/other\" }\n"
    );
}

#[test]
fn path_and_git_dependencies_parse_by_the_language_servers_rules() {
    let root = Path::new("/project");
    let (dependencies, _) = manifest_dependencies(
        root,
        "[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\nVendorLib = { git = \"file:///tmp/vendor\", tag = \"v1\", version = \"1.0.0\" }\n",
    )
    .expect("a manifest");
    assert_eq!(dependencies.len(), 2);
    assert_eq!(dependencies[0].name, "OSCAT");
    assert_eq!(
        dependencies[0].path.as_deref(),
        Some(Path::new("/project/libraries/oscat"))
    );
    assert_eq!(dependencies[0].version.as_deref(), Some("0.1.0"));
    let git = dependencies[1].git.as_ref().expect("a Git dependency");
    assert_eq!(dependencies[1].name, "VendorLib");
    assert_eq!(git.url, "file:///tmp/vendor");
    assert_eq!(git.tag.as_deref(), Some("v1"));
    assert_eq!(dependencies[1].version.as_deref(), Some("1.0.0"));
}

#[test]
fn a_git_dependency_has_at_most_one_pin_selector() {
    let branch = DependencySpec {
        git: Some("file:///tmp/vendor".to_owned()),
        branch: Some("main".to_owned()),
        version: Some("1.0.0".to_owned()),
        ..DependencySpec::default()
    };
    assert_eq!(
        upsert("", "VendorLib", &branch).expect("upsert"),
        "[dependencies]\nVendorLib = { git = \"file:///tmp/vendor\", branch = \"main\", version = \"1.0.0\" }\n"
    );
    let two_pins = DependencySpec {
        git: Some("file:///tmp/vendor".to_owned()),
        tag: Some("v1".to_owned()),
        rev: Some("abc".to_owned()),
        ..DependencySpec::default()
    };
    let error = upsert("", "VendorLib", &two_pins).expect_err("two pins");
    assert_eq!(
        error,
        "Git dependency may set only one of rev, tag, or branch."
    );
    let neither = DependencySpec::default();
    assert_eq!(
        upsert("", "Lib", &neither).expect_err("no source"),
        "Library dependency must set exactly one of path or git."
    );
}

#[test]
fn git_pins_are_classified_and_paths_use_forward_slashes() {
    // A version is a tag, a 7 to 40 digit hexadecimal text a commit, anything else a branch.
    assert_eq!(
        classify_git_pin("v1.2.3"),
        Some(("tag", "v1.2.3".to_owned()))
    );
    assert_eq!(
        classify_git_pin("feature/main"),
        Some(("branch", "feature/main".to_owned()))
    );
    assert_eq!(
        classify_git_pin("0123456789abcdef"),
        Some(("rev", "0123456789abcdef".to_owned()))
    );
    assert_eq!(classify_git_pin("2.0"), Some(("tag", "2.0".to_owned())));
    assert_eq!(
        classify_git_pin("abc1234"),
        Some(("rev", "abc1234".to_owned()))
    );
    assert_eq!(classify_git_pin("1.x"), Some(("branch", "1.x".to_owned())));
    assert_eq!(classify_git_pin("   "), None);
    assert_eq!(
        relative_posix(Path::new("/p"), Path::new("/p/libraries/oscat")),
        "libraries/oscat"
    );
    assert_eq!(
        relative_posix(Path::new("/p/app"), Path::new("/p/shared/lib")),
        "../shared/lib"
    );
}

#[test]
fn the_package_version_is_read_from_the_library_manifest() {
    let folder =
        std::env::temp_dir().join(format!("trust-lsp-package-version-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&folder);
    std::fs::create_dir_all(&folder).expect("folder");
    std::fs::write(
        folder.join(MANIFEST_FILE),
        "[package]\nversion = \"0.1.0\"\n",
    )
    .expect("manifest");
    let version = package_version(&folder);
    let _ = std::fs::remove_dir_all(&folder);
    assert_eq!(version.as_deref(), Some("0.1.0"));
    assert_eq!(package_version(Path::new("/nonexistent/library")), None);
}

#[test]
fn a_new_entry_goes_to_the_end_of_dependencies_before_the_next_table() {
    // Catches: an entry appended at the end of the file, inside another table.
    let manifest = "[dependencies]\nA = { path = \"a\" }\n\n[build]\nprofile = \"debug\"\n";
    assert_eq!(
        upsert(manifest, "B", &DependencySpec::path("b".to_owned(), None)).expect("upsert"),
        "[dependencies]\nA = { path = \"a\" }\nB = { path = \"b\" }\n\n[build]\nprofile = \"debug\"\n"
    );
}

#[test]
fn a_comment_above_an_updated_entry_stays() {
    // Catches: an update that drops what the user wrote about the dependency.
    let manifest =
        "[dependencies]\n# our vendored copy\nOSCAT = { path = \"x\", version = \"0.0.1\" }\n";
    assert_eq!(
        upsert(manifest, "OSCAT", &path_spec("libraries/oscat", "0.1.0")).expect("upsert"),
        "[dependencies]\n# our vendored copy\nOSCAT = { path = \"libraries/oscat\", version = \"0.1.0\" }\n"
    );
}

#[test]
fn a_dependency_written_as_its_own_table_is_replaced_and_removed() {
    // The language server reads `[dependencies.OSCAT]` as well as an inline entry, so the panel edits
    // both. Catches: a second OSCAT entry beside the table.
    let manifest = "[dependencies.OSCAT]\npath = \"old\"\n";
    let updated =
        upsert(manifest, "OSCAT", &path_spec("libraries/oscat", "0.1.0")).expect("upsert");
    let (dependencies, _) = manifest_dependencies(Path::new("/p"), &updated).expect("manifest");
    assert_eq!(dependencies.len(), 1);
    assert_eq!(
        dependencies[0].path.as_deref(),
        Some(Path::new("/p/libraries/oscat"))
    );
    let removed = remove(manifest, "OSCAT").expect("remove");
    let (dependencies, _) = manifest_dependencies(Path::new("/p"), &removed).expect("manifest");
    assert!(dependencies.is_empty(), "{removed}");
}

#[test]
fn removing_an_absent_dependency_changes_nothing_and_an_unreadable_manifest_is_refused() {
    let manifest = "include_paths = [\"src\"]\n\n[dependencies]\nA = \"libs/a\"\n";
    assert_eq!(remove(manifest, "B").expect("remove"), manifest);
    assert_eq!(
        remove("include_paths = [\"src\"]\n", "A").expect("remove"),
        "include_paths = [\"src\"]\n"
    );
    let error = upsert(
        "[dependencies\n",
        "A",
        &DependencySpec::path("a".to_owned(), None),
    )
    .expect_err("unreadable");
    assert!(
        error.starts_with("The project's trust-lsp.toml cannot be read: "),
        "{error}"
    );
}
