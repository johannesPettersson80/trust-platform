use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use super::*;

static NEXT_FIXTURE: AtomicU64 = AtomicU64::new(1);

struct TempTree {
    root: PathBuf,
}

impl TempTree {
    fn new(label: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-web-ide-fs-{}-{label}-{}",
            std::process::id(),
            NEXT_FIXTURE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&root).expect("create root");
        Self { root }
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.root.join(relative);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("create parent");
        }
        std::fs::write(path, text).expect("write file");
    }
}

impl Drop for TempTree {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

#[test]
fn workspace_file_collection_returns_relative_forward_slash_paths() {
    let tree = TempTree::new("relative");
    tree.write("main.st", "");
    tree.write("nested/child.txt", "");
    let mut files = Vec::new();
    collect_workspace_files(&tree.root, Path::new(""), &mut files).expect("collect");
    files.sort();
    assert_eq!(
        files,
        vec!["main.st".to_string(), "nested/child.txt".to_string()]
    );
}

#[test]
fn workspace_file_collection_recurses_multiple_levels() {
    let tree = TempTree::new("recursive");
    tree.write("a/b/c/main.st", "");
    let mut files = Vec::new();
    collect_workspace_files(&tree.root, Path::new(""), &mut files).expect("collect");
    assert_eq!(files, vec!["a/b/c/main.st".to_string()]);
}

#[test]
fn hidden_files_and_directories_are_omitted() {
    let tree = TempTree::new("hidden");
    tree.write(".secret.st", "");
    tree.write(".git/config", "");
    tree.write("visible.st", "");
    let mut files = Vec::new();
    collect_workspace_files(&tree.root, Path::new(""), &mut files).expect("collect");
    assert_eq!(files, vec!["visible.st".to_string()]);
}

#[test]
fn source_collection_filters_extension_case_insensitively() {
    let tree = TempTree::new("sources");
    tree.write("a.st", "");
    tree.write("b.ST", "");
    tree.write("c.St", "");
    tree.write("d.txt", "");
    let mut files = Vec::new();
    collect_source_files(&tree.root, Path::new(""), &mut files).expect("collect");
    files.sort();
    assert_eq!(
        files,
        vec!["a.st".to_string(), "b.ST".to_string(), "c.St".to_string()]
    );
}

#[test]
fn missing_collection_root_returns_typed_internal_error() {
    let tree = TempTree::new("missing");
    let mut files = Vec::new();
    let error = collect_workspace_files(&tree.root, Path::new("absent"), &mut files)
        .expect_err("missing directory");
    assert_eq!(error.kind(), IdeErrorKind::Internal);
    assert!(files.is_empty());
}

#[test]
fn workspace_tree_sorts_siblings_by_name() {
    let tree = TempTree::new("sort");
    tree.write("zeta.st", "");
    tree.write("alpha.st", "");
    tree.write("middle/file.st", "");
    let nodes = collect_workspace_tree(&tree.root, Path::new("")).expect("tree");
    assert_eq!(
        nodes
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha.st", "middle", "zeta.st"]
    );
}

#[test]
fn workspace_tree_projects_directory_children_and_kinds() {
    let tree = TempTree::new("kinds");
    tree.write("nested/child.st", "");
    let nodes = collect_workspace_tree(&tree.root, Path::new("")).expect("tree");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].kind, "directory");
    assert_eq!(nodes[0].path, "nested");
    assert_eq!(nodes[0].children.len(), 1);
    assert_eq!(nodes[0].children[0].kind, "file");
    assert_eq!(nodes[0].children[0].path, "nested/child.st");
}

#[test]
fn workspace_tree_omits_hidden_entries_at_every_level() {
    let tree = TempTree::new("tree-hidden");
    tree.write(".top.st", "");
    tree.write("visible/.nested.st", "");
    tree.write("visible/main.st", "");
    let nodes = collect_workspace_tree(&tree.root, Path::new("")).expect("tree");
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].children.len(), 1);
    assert_eq!(nodes[0].children[0].name, "main.st");
}

#[cfg(unix)]
#[test]
fn source_collection_does_not_follow_directory_symlink_outside_root() {
    let tree = TempTree::new("symlink-root");
    let outside = TempTree::new("symlink-outside");
    outside.write("escaped.st", "");
    std::os::unix::fs::symlink(&outside.root, tree.root.join("linked"))
        .expect("create directory symlink");

    let mut files = Vec::new();
    collect_source_files(&tree.root, Path::new(""), &mut files).expect("collect");
    assert!(!files.iter().any(|path| path.contains("escaped.st")));
}

#[test]
fn empty_workspace_tree_is_empty() {
    let tree = TempTree::new("empty");
    assert!(collect_workspace_tree(&tree.root, Path::new(""))
        .expect("empty tree")
        .is_empty());
}
