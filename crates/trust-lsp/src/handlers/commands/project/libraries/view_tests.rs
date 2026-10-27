//! The Libraries panel's entries (specification 25 §7: installed and update state come from the
//! project's vendored copy, preferring its package version over stale project information). Each
//! test writes its project and bundled libraries.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-libraries-view-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, text).expect("file");
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn find<'a>(entries: &'a [Value], name: &str) -> &'a Value {
    entries
        .iter()
        .find(|entry| entry["name"] == json!(name))
        .unwrap_or_else(|| panic!("no entry {name}: {entries:?}"))
}

#[test]
fn a_vendored_curated_library_shows_its_copys_version_and_the_bundled_update() {
    // The manifest says 0.0.9 and the language server reports 0.0.8, the vendored copy is 0.1.0 and
    // the extension bundles 0.2.0. Catches: stale project information shown as the installed version,
    // or no update offered.
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nOSCAT = { path = \"libraries/oscat\", version = \"0.0.9\" }\n",
    );
    scratch.write(
        "project/libraries/oscat/trust-lsp.toml",
        "[package]\nversion = \"0.1.0\"\n",
    );
    scratch.write(
        "project/libraries/oscat/src/basic.st",
        "FUNCTION INC : INT\nEND_FUNCTION\nFUNCTION_BLOCK Averager\nEND_FUNCTION_BLOCK\n",
    );
    scratch.write(
        "bundled/oscat/trust-lsp.toml",
        "[package]\nversion = \"0.2.0\"\n",
    );
    let resolved = [ResolvedLibrary {
        name: "OSCAT".to_owned(),
        version: Some("0.0.8".to_owned()),
        path: Some(scratch.0.join("elsewhere")),
    }];
    let entries = entries(
        &scratch.0.join("project"),
        &scratch.0.join("bundled"),
        &resolved,
    )
    .expect("view");
    let oscat = find(&entries, "OSCAT");
    assert_eq!(oscat["label"], json!("OSCAT"));
    assert_eq!(oscat["source"], json!("bundled"));
    assert_eq!(oscat["status"], json!("resolved"));
    assert_eq!(oscat["version"], json!("0.1.0"));
    assert_eq!(
        oscat["updateAvailable"],
        json!({ "current": "0.1.0", "next": "0.2.0", "curatedId": "oscat" })
    );
    assert_eq!(oscat["detail"], json!("2 symbols available"));
    assert_eq!(
        oscat["symbols"][1]["snippet"],
        json!("averager : Averager;")
    );
    assert_eq!(oscat["canFixPath"], json!(false));
}

#[test]
fn a_missing_local_folder_fails_and_can_be_fixed_and_git_waits_for_the_resolver() {
    // Catches: a missing path shown as resolved, or Fix path offered for a Git library.
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nGone = { path = \"../gone\" }\nVendorLib = { git = \"file:///tmp/vendor\", tag = \"v1\" }\n",
    );
    let entries =
        entries(&scratch.0.join("project"), &scratch.0.join("bundled"), &[]).expect("view");
    let gone = find(&entries, "Gone");
    assert_eq!(gone["source"], json!("local"));
    assert_eq!(gone["status"], json!("failed"));
    assert_eq!(
        gone["detail"],
        json!("Path is missing. Fix or remove this library.")
    );
    assert_eq!(gone["canFixPath"], json!(true));
    assert_eq!(gone["symbols"], json!([]));
    let vendor = find(&entries, "VendorLib");
    assert_eq!(vendor["source"], json!("git"));
    assert_eq!(vendor["status"], json!("resolving"));
    assert_eq!(
        vendor["detail"],
        json!("Waiting for the dependency resolver to pin this Git library.")
    );
    assert_eq!(vendor["canFixPath"], json!(false));
    assert_eq!(vendor["path"], Value::Null);
}

#[test]
fn another_library_prefers_the_language_servers_resolution() {
    // A Git library resolved into a checkout, and a local library whose resolution names 2.0.0 while
    // its manifest says 1.0.0. Catches: a resolved Git library shown as waiting, or the manifest's
    // version preferred over the resolution for a library that is not curated.
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nVendorLib = { git = \"file:///tmp/vendor\", tag = \"v1\" }\nLocal = { path = \"../local\" }\n",
    );
    scratch.write(
        "checkout/trust-lsp.toml",
        "[package]\nversion = \"1.1.0\"\n",
    );
    scratch.write("checkout/src/one.st", "FUNCTION One : INT\nEND_FUNCTION\n");
    scratch.write("local/trust-lsp.toml", "[package]\nversion = \"1.0.0\"\n");
    let resolved = [
        ResolvedLibrary {
            name: "VendorLib".to_owned(),
            version: None,
            path: Some(scratch.0.join("checkout")),
        },
        ResolvedLibrary {
            name: "Local".to_owned(),
            version: Some("2.0.0".to_owned()),
            path: None,
        },
    ];
    let entries = entries(
        &scratch.0.join("project"),
        &scratch.0.join("bundled"),
        &resolved,
    )
    .expect("view");
    let vendor = find(&entries, "VendorLib");
    assert_eq!(vendor["status"], json!("resolved"));
    assert_eq!(vendor["version"], json!("1.1.0"));
    assert_eq!(vendor["detail"], json!("1 symbol available"));
    let local = find(&entries, "Local");
    assert_eq!(local["status"], json!("resolved"));
    assert_eq!(local["version"], json!("2.0.0"));
}

#[test]
fn entries_are_ordered_by_label_and_an_unreadable_manifest_is_reported() {
    let scratch = Scratch::new();
    scratch.write(
        "project/trust-lsp.toml",
        "[dependencies]\nZeta = \"zeta\"\nPLCopenMotionSingleAxis = \"libraries/plcopen_motion\"\nalpha = \"alpha\"\n",
    );
    let listed =
        entries(&scratch.0.join("project"), &scratch.0.join("bundled"), &[]).expect("view");
    let labels: Vec<_> = listed.iter().map(|entry| entry["label"].clone()).collect();
    assert_eq!(
        labels,
        [json!("alpha"), json!("PLCopen Motion"), json!("Zeta")]
    );
    scratch.write("project/trust-lsp.toml", "[dependencies\n");
    assert!(entries(&scratch.0.join("project"), &scratch.0.join("bundled"), &[]).is_err());
}
