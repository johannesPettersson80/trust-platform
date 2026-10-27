//! The project Compile checks (specification 25 §6.2). The folders are written by each test.

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-project-root-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("scratch");
        Self(path)
    }

    fn file(&self, relative: &str) -> PathBuf {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(&path, "").expect("file");
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn root(path: Option<&Path>, folders: &[PathBuf]) -> Option<String> {
    let answer = answer(json!({ "path": path, "folders": folders })).expect("answer");
    answer["root"].as_str().map(str::to_owned)
}

#[test]
fn the_nearest_manifest_above_the_active_file_wins() {
    // A workspace folder holding two projects: the active file is in `plant/src`, whose project is
    // `plant`, not the workspace folder. Catches: Compile checking the workspace folder.
    let workspace = Scratch::new();
    workspace.file("trust-lsp.toml");
    workspace.file("plant/trust-lsp.toml");
    let main = workspace.file("plant/src/Main.st");
    assert_eq!(
        root(Some(&main), std::slice::from_ref(&workspace.0)),
        Some(workspace.0.join("plant").display().to_string())
    );
}

#[test]
fn without_a_manifest_the_active_files_workspace_folder_is_checked() {
    // Two workspace folders, no manifest: the second holds the active file. Catches: the first
    // folder chosen although the file belongs to another.
    let first = Scratch::new();
    let second = Scratch::new();
    let main = second.file("src/Main.st");
    assert_eq!(
        root(Some(&main), &[first.0.clone(), second.0.clone()]),
        Some(second.0.display().to_string())
    );
}

#[test]
fn without_an_active_file_the_first_folder_is_checked_and_none_without_folders() {
    let first = Scratch::new();
    let second = Scratch::new();
    assert_eq!(
        root(None, &[first.0.clone(), second.0.clone()]),
        Some(first.0.display().to_string())
    );
    assert_eq!(root(None, &[]), None);
}

#[test]
fn the_runtime_project_is_the_folder_with_runtime_toml_else_its_bundle_folder() {
    // Catches: attach or the runtime connection looking for runtime.toml in the wrong folder.
    let direct = Scratch::new();
    direct.file("runtime.toml");
    direct.file("bundle/runtime.toml");
    let bundled = Scratch::new();
    bundled.file("bundle/runtime.toml");
    let none = Scratch::new();
    none.file("src/Main.st");
    let runtime_project = |folder: &Path| {
        answer(json!({ "runtimeFolder": folder })).expect("answer")["runtimeProject"].clone()
    };
    assert_eq!(
        runtime_project(&direct.0),
        json!(direct.0.display().to_string())
    );
    assert_eq!(
        runtime_project(&bundled.0),
        json!(bundled.0.join("bundle").display().to_string())
    );
    assert_eq!(runtime_project(&none.0), Value::Null);
}
