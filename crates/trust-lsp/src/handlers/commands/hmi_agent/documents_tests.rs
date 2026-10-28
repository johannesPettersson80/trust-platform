//! Tests of the document snapshot rule (specification 43, §4 and §8 repair 2). Port of
//! `editors/vscode/src/test/suite/hmi-agent-documents.test.ts`, and of the byte-order-mark assertion
//! of `hmi-agent-apply.test.ts`.
//!
//! The VS Code editor of the TypeScript tests is replaced by an `OpenDocument` fixture, which is what the
//! extension passes. Not ported: the remote URI authority case (this module never receives a URI) and
//! the cancellation case (cancellation is the caller's check before it calls this module).

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::super::error::{INVALID_ARGUMENT, LIMIT_EXCEEDED, PROJECT_UNAVAILABLE};
use super::*;

/// `sha256("absent\0")`: the revision digest of a page that does not exist.
const ABSENT: &str = "b7c03610089b9f660990ee7db3290cc8d8564161b079319c9c7ba1f19dc2e190";
/// `sha256("present\0")`: the revision digest of an existing empty page.
const EMPTY_PRESENT: &str = "4c3e07e2f59edac51e01f844f97a23e7c5f8da45a4a748f62e0ea49336e28557";
/// `sha256("present\0saved")`.
const SAVED_PRESENT: &str = "07943712d0c7a539074089bae8344ba451ecef884355cf0cc59f4a47af67af2e";
/// `sha256("present\0other page")`.
const OTHER_PAGE_PRESENT: &str = "1b571e0f4ae1c533127e8a8112233f654fec0864182807ad9286976fde3f1194";

/// A fresh workspace folder under the system temporary folder, removed when the test ends.
struct Workspace(PathBuf);

impl Workspace {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "trust-hmi-documents-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::SeqCst)
        ));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(path.join("hmi/pages")).expect("create the workspace folder");
        Self(path)
    }

    fn root(&self) -> &Path {
        &self.0
    }

    fn write(&self, file: &str, text: &str) {
        let path = self.0.join(file);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create the folder of the page");
        }
        fs::write(path, text).expect("write the page");
    }
}

impl Drop for Workspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

// Port of "uses dirty editor text and changes revision even before save" (hmi-agent-documents.test.ts;
// specification 43 §4: a dirty buffer is authoritative until it is saved or discarded).
#[test]
fn a_dirty_buffer_is_the_text_and_changes_the_revision_before_save() {
    let workspace = Workspace::new("dirty");
    let file = "hmi/pages/overview.toml";
    workspace.write(file, "saved");
    let before = snapshot(workspace.root(), file, None).expect("read the saved page");
    assert_eq!(before.text, "saved");
    assert_eq!(before.revision, format!("disk:{SAVED_PRESENT}"));
    let buffer = OpenDocument {
        text: "manual edit".to_string(),
        version: 2,
        dirty: true,
    };
    let after = snapshot(workspace.root(), file, Some(&buffer)).expect("read the dirty buffer");
    assert_eq!(after.text, "manual edit");
    assert!(after.dirty);
    assert_eq!(after.version, Some(2));
    assert!(
        after.revision.starts_with("2:"),
        "the revision names the editor version"
    );
    assert_ne!(after.revision, before.revision);
    assert_eq!(
        fs::read_to_string(workspace.root().join(file)).expect("read the disk"),
        "saved",
        "an unsaved buffer leaves the disk as it is"
    );
}

// Port of "missing and existing empty files have different revisions" (hmi-agent-documents.test.ts).
#[test]
fn a_missing_page_and_an_existing_empty_page_have_different_revisions() {
    let workspace = Workspace::new("missing");
    let file = "hmi/pages/new.toml";
    let missing = snapshot(workspace.root(), file, None).expect("read a missing page");
    assert!(!missing.exists);
    assert_eq!(missing.revision, format!("disk:{ABSENT}"));
    workspace.write(file, "");
    let present = snapshot(workspace.root(), file, None).expect("read an empty page");
    assert!(present.exists);
    assert_eq!(present.revision, format!("disk:{EMPTY_PRESENT}"));
    assert_ne!(present.revision, missing.revision);
    assert!(
        !same_content(&missing, &present),
        "an absent page and an empty page hold different content"
    );
}

// Port of "rejects traversal, absolute paths and another URI authority" (hmi-agent-documents.test.ts).
// The remote authority case is not ported: a `vscode-remote` URI never reaches this module.
#[test]
fn rejects_traversal_absolute_and_uri_like_paths() {
    let workspace = Workspace::new("paths");
    for file in [
        "../escape",
        "/tmp/outside",
        "hmi/../../escape",
        "file:///tmp/outside",
        "hmi\\..\\outside",
        "hmi//pages/a",
        "hmi/./pages/a",
        "",
    ] {
        let error = snapshot(workspace.root(), file, None).expect_err(file);
        assert_eq!(error.code, INVALID_ARGUMENT, "{file}");
        assert_eq!(
            error.message, "file must be a normalized workspace-relative path.",
            "{file}"
        );
    }
}

// Port of "rejects symlink escape including a new file below an escaped directory"
// (hmi-agent-documents.test.ts).
#[cfg(unix)]
#[test]
fn rejects_a_symbolic_link_escape_including_a_new_page_below_it() {
    let workspace = Workspace::new("symlink");
    let outside = Workspace::new("outside");
    std::os::unix::fs::symlink(outside.root(), workspace.root().join("hmi/escaped"))
        .expect("create the symbolic link");
    let error = snapshot(workspace.root(), "hmi/escaped/new.toml", None)
        .expect_err("a page below an escaped folder");
    assert_eq!(error.code, PROJECT_UNAVAILABLE);
    assert_eq!(error.message, "path escapes the workspace.");
}

// A dangling symbolic link is refused rather than followed to a page that a later write would create
// outside the workspace (specification 43, §4: symlink escape).
#[cfg(unix)]
#[test]
fn a_dangling_symbolic_link_is_refused_and_not_followed() {
    let workspace = Workspace::new("dangling");
    let target = std::env::temp_dir().join(format!(
        "trust-hmi-dangling-target-{}-missing",
        std::process::id()
    ));
    std::os::unix::fs::symlink(&target, workspace.root().join("hmi/dangling"))
        .expect("create the symbolic link");
    let error =
        snapshot(workspace.root(), "hmi/dangling/new.toml", None).expect_err("a dangling link");
    assert_eq!(error.code, PROJECT_UNAVAILABLE);
    assert_eq!(error.message, "path escapes the workspace.");
}

// Port of "does not treat permission or invalid-path errors as a missing file"
// (hmi-agent-documents.test.ts): a page used as a folder is an error, not a missing page.
#[test]
fn an_invalid_path_is_an_error_and_not_a_missing_page() {
    let workspace = Workspace::new("notdir");
    workspace.write("hmi/pages/overview.toml", "saved");
    let error = snapshot(workspace.root(), "hmi/pages/overview.toml/child", None)
        .expect_err("a page is not a folder");
    assert_eq!(error.code, PROJECT_UNAVAILABLE);
}

// Port of "enforces per-file bytes" (hmi-agent-documents.test.ts). The cancellation half of that test
// is not ported.
#[test]
fn a_page_is_limited_to_one_mebibyte_on_disk_and_in_a_buffer() {
    let workspace = Workspace::new("size");
    workspace.write("hmi/pages/large.toml", &"é".repeat(600_000));
    let error =
        snapshot(workspace.root(), "hmi/pages/large.toml", None).expect_err("a 1.2 MB page");
    assert_eq!(error.code, LIMIT_EXCEEDED);
    assert_eq!(error.message, "file exceeds 1 MiB.");
    // The limit is inclusive: exactly 1 MiB is read, one byte more is refused.
    workspace.write("hmi/pages/exact.toml", &"a".repeat(1_048_576));
    let exact = snapshot(workspace.root(), "hmi/pages/exact.toml", None).expect("exactly 1 MiB");
    assert_eq!(exact.text.len(), 1_048_576);
    workspace.write("hmi/pages/over.toml", &"a".repeat(1_048_577));
    let over = snapshot(workspace.root(), "hmi/pages/over.toml", None).expect_err("1 MiB plus 1");
    assert_eq!(over.code, LIMIT_EXCEEDED);
    // A buffer is measured in UTF-8 bytes, as the disk is.
    let buffer = OpenDocument {
        text: "é".repeat(600_000),
        version: 1,
        dirty: true,
    };
    let error = snapshot(workspace.root(), "hmi/pages/overview.toml", Some(&buffer))
        .expect_err("a 1.2 MB buffer");
    assert_eq!(error.code, LIMIT_EXCEEDED);
}

// Port of "a clean cached editor model does not resurrect a file deleted from disk"
// (hmi-agent-documents.test.ts; specification 43 §4).
#[test]
fn a_clean_cached_document_does_not_resurrect_a_deleted_page() {
    let workspace = Workspace::new("deleted");
    let file = "hmi/pages/overview.toml";
    workspace.write(file, "saved");
    let clean = OpenDocument {
        text: "saved".to_string(),
        version: 1,
        dirty: false,
    };
    let open = snapshot(workspace.root(), file, Some(&clean)).expect("read the clean editor copy");
    assert!(open.exists);
    assert_eq!(open.version, Some(1));
    assert_eq!(open.revision, format!("1:{SAVED_PRESENT}"));
    fs::remove_file(workspace.root().join(file)).expect("delete the page");
    let deleted = snapshot(workspace.root(), file, Some(&clean)).expect("read the deleted page");
    assert!(
        !deleted.exists,
        "a deleted clean model is not an existing source file"
    );
    assert_eq!(deleted.text, "");
    assert_eq!(deleted.version, None);
    assert_eq!(deleted.revision, format!("disk:{ABSENT}"));
}

// Port of "a byte order mark on disk is not a change once the page is opened" (hmi-agent-apply.test.ts):
// the disk text is read the way the editor model holds it.
#[test]
fn a_byte_order_mark_on_disk_is_not_part_of_the_page_text() {
    let workspace = Workspace::new("bom");
    let file = "hmi/pages/a.toml";
    workspace.write(file, "\u{feff}original");
    let observed = snapshot(workspace.root(), file, None).expect("read the page");
    assert_eq!(observed.text, "original");
    // Only the first byte order mark is dropped.
    workspace.write(file, "\u{feff}\u{feff}original");
    let second = snapshot(workspace.root(), file, None).expect("read the page");
    assert_eq!(second.text, "\u{feff}original");
}

// Repair 2 of 7 October 2026 (hmi-agent-apply.test.ts, "opening the prepared pages without changing
// them does not block application"): opening a page changes its observed revision, not its text.
#[test]
fn opening_a_page_without_changing_its_text_is_no_conflict() {
    let workspace = Workspace::new("opening");
    let file = "hmi/pages/a.toml";
    workspace.write(file, "other page");
    let on_disk = snapshot(workspace.root(), file, None).expect("read the page");
    let opened = OpenDocument {
        text: "other page".to_string(),
        version: 3,
        dirty: false,
    };
    let observed = snapshot(workspace.root(), file, Some(&opened)).expect("read the opened page");
    assert_eq!(observed.revision, format!("3:{OTHER_PAGE_PRESENT}"));
    assert_ne!(
        observed.revision, on_disk.revision,
        "opening changes the observed revision"
    );
    assert!(
        same_content(&on_disk, &observed),
        "opening does not change the text"
    );
    let changed = OpenDocument {
        text: "customer change".to_string(),
        version: 4,
        dirty: true,
    };
    let edited = snapshot(workspace.root(), file, Some(&changed)).expect("read the buffer");
    assert!(
        !same_content(&on_disk, &edited),
        "changed text is a difference"
    );
}

// A page in a folder that does not exist yet is a missing page, not an error (specification 43 §4).
#[test]
fn a_page_in_a_folder_that_does_not_exist_yet_is_missing() {
    let workspace = Workspace::new("newfolder");
    let observed = snapshot(workspace.root(), "hmi/new/page.toml", None).expect("read the page");
    assert!(!observed.exists);
    assert_eq!(observed.revision, format!("disk:{ABSENT}"));
    let inside = resolve(workspace.root(), "hmi/new/page.toml").expect("a normalized path");
    assert_eq!(
        inside,
        workspace.root().join("hmi").join("new").join("page.toml")
    );
}

// A folder at a page's path is not a page (specification 43 §4).
#[test]
fn a_folder_is_not_a_page() {
    let workspace = Workspace::new("folder");
    fs::create_dir_all(workspace.root().join("hmi/pages/folder.toml")).expect("create the folder");
    let error = snapshot(workspace.root(), "hmi/pages/folder.toml", None).expect_err("a folder");
    assert_eq!(error.code, INVALID_ARGUMENT);
    assert_eq!(error.message, "expected a file.");
}

// The digest helper matches the published SHA-256 test vectors.
#[test]
fn sha256_matches_the_published_vectors() {
    assert_eq!(
        sha256_hex(b""),
        "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
    );
    assert_eq!(
        sha256_hex(b"abc"),
        "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
    );
}
