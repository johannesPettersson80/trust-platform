//! Private copies of the repository's examples for the tests that run `trust-runtime` on them: a fresh
//! root per copy, repository paths joined one component at a time, a tree copied without its build
//! outputs, and a source edited in the copy. Split from `runtime_process.rs`, which starts and drives
//! the runtimes on these copies.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use super::repo_root;

/// Entries left out of the copy: programs and build folders of earlier builds and dependency caches.
const SKIPPED: [&str; 6] = [
    "program.tpi",
    "deployment-record.bin",
    "build",
    "target",
    ".trust",
    "node_modules",
];

static COPY_COUNTER: AtomicU64 = AtomicU64::new(1);

/// A fresh root holding a private copy of `examples/<example>` at that same relative path, without
/// its build outputs, for a test that runs `trust-runtime` commands from the root as a reader runs
/// them from the repository root. The caller removes the root.
pub fn copy_example(example: &str) -> PathBuf {
    let root = unique_root();
    let path = format!("examples/{example}");
    copy_tree(
        &join_relative(&repo_root(), &path),
        &join_relative(&root, &path),
    );
    root
}

pub(super) fn unique_root() -> PathBuf {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time before unix epoch")
        .as_nanos();
    let sequence = COPY_COUNTER.fetch_add(1, Ordering::Relaxed);
    let root = trust_test_support::unique_temp_path(&format!(
        "trust-runtime-ctl-{}-{nanos}-{sequence}",
        std::process::id()
    ));
    fs::create_dir_all(&root).expect("create runtime test directory");
    root
}

/// Joins a `/`-separated repository path onto `base`, one component at a time.
pub(super) fn join_relative(base: &Path, relative: &str) -> PathBuf {
    relative
        .split('/')
        .fold(base.to_path_buf(), |path, part| path.join(part))
}

/// Copies a directory tree, leaving out build outputs.
pub(super) fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).expect("create copied directory");
    let entries = fs::read_dir(from).unwrap_or_else(|err| panic!("read {}: {err}", from.display()));
    for entry in entries {
        let entry = entry.expect("read directory entry");
        let name = entry.file_name().to_string_lossy().into_owned();
        if SKIPPED.contains(&name.as_str()) {
            continue;
        }
        let source = entry.path();
        let target = to.join(&name);
        if entry.file_type().expect("file type").is_dir() {
            copy_tree(&source, &target);
        } else {
            fs::copy(&source, &target)
                .unwrap_or_else(|err| panic!("copy {}: {err}", source.display()));
        }
    }
}

pub(super) fn replace_in_file(path: &Path, from: &str, to: &str) {
    let text =
        fs::read_to_string(path).unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
    // A Windows checkout can convert the example sources to CRLF line endings, while the edits are
    // written with LF; Structured Text accepts either, so the copy is normalized to LF first.
    let text = text.replace("\r\n", "\n");
    assert!(
        text.contains(from),
        "{} has no `{from}` to edit",
        path.display()
    );
    fs::write(path, text.replace(from, to)).expect("write edited source");
}
