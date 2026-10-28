//! The bundled libraries an HMI installs (specification 39, §3.1 item 9, §4.10; specification 25
//! §7).

use std::path::{Path, PathBuf};

use serde_json::{json, Value};

use super::answer;

/// A fresh folder for one test, removed when the test ends.
struct Folder(PathBuf);

impl Folder {
    fn new(name: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-hmi-libraries-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let file = self.0.join(relative);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
}

impl Drop for Folder {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The extension's collection of the integration test: Plant needs OscatOop, which needs OSCAT.
fn collection(folder: &Folder) -> PathBuf {
    folder.write(
        "extension/plant/trust-lsp.toml",
        "[package]\nname=\"Plant\"\nversion=\"0.1.0\"\n[dependencies]\nOscatOop={path=\"../oscat/oop\",version=\"0.1.0\"}\n",
    );
    folder.write(
        "extension/oscat/oop/trust-lsp.toml",
        "[package]\nname=\"OscatOop\"\nversion=\"0.1.0\"\n[dependencies]\nOSCAT={path=\"..\",version=\"0.1.0\"}\n",
    );
    folder.write(
        "extension/oscat/trust-lsp.toml",
        "[package]\nname=\"OSCAT\"\nversion=\"0.1.0\"\n",
    );
    folder.write(
        "extension/plant/src/pump.st",
        "FUNCTION_BLOCK Pump END_FUNCTION_BLOCK\n",
    );
    folder.write(
        "extension/plant/examples/demo/src/Main.st",
        "PROGRAM Main END_PROGRAM\n",
    );
    folder.0.join("extension")
}

fn install(project: &Path, bundled: &Path, libraries: Value, open: Value) -> Result<Value, String> {
    answer(json!({ "root": project, "bundled": bundled, "libraries": libraries, "open": open }))
}

fn created(answer: &Value) -> Vec<String> {
    answer["create"]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn plant_brings_its_nested_dependencies_without_its_examples_and_names_only_itself() {
    // A package missing its OSCAT dependencies would not compile in the project; one carrying its
    // example projects would add their programs to the project's own.
    let folder = Folder::new("nested");
    let bundled = collection(&folder);
    folder.write(
        "project/trust-lsp.toml",
        "[project]\ninclude_paths = [\"src\"]\n",
    );
    let project = folder.0.join("project");
    let answer = install(
        &project,
        &bundled,
        json!([{ "name": "Plant", "folder": "plant" }]),
        json!({}),
    )
    .unwrap();
    assert_eq!(
        created(&answer),
        [
            "libraries/oscat/oop/trust-lsp.toml",
            "libraries/oscat/trust-lsp.toml",
            "libraries/plant/src/pump.st",
            "libraries/plant/trust-lsp.toml",
        ]
    );
    assert_eq!(
        answer["create"][2]["text"],
        json!("FUNCTION_BLOCK Pump END_FUNCTION_BLOCK\n"),
        "the bundled text, unchanged"
    );
    let manifest = answer["manifest"]["text"].as_str().unwrap();
    assert!(
        manifest.starts_with("[project]\ninclude_paths = [\"src\"]\n")
            && manifest.contains("Plant = { path = \"libraries/plant\", version = \"0.1.0\" }")
            && !manifest.contains("Oscat"),
        "{manifest}"
    );
    assert_eq!(answer["manifest"]["path"], json!("trust-lsp.toml"));
    assert!(
        !project.join("libraries").exists(),
        "nothing is written; the extension applies the answer in one edit"
    );
    // What was read, so the extension can see that nothing changed before it applies the edit: the
    // manifest by the SHA-256 of its text, each library file that did not exist as null.
    let checked = answer["checked"].as_array().unwrap();
    assert_eq!(checked.len(), 5, "the manifest and each library file");
    let manifest_digest = hex::encode(<sha2::Sha256 as sha2::Digest>::digest(
        b"[project]\ninclude_paths = [\"src\"]\n",
    ));
    assert!(checked.contains(&json!({ "path": "trust-lsp.toml", "sha256": manifest_digest })));
    assert!(checked.contains(&json!({ "path": "libraries/plant/src/pump.st", "sha256": null })));
}

#[test]
fn a_library_the_project_names_already_is_left_and_the_manifest_kept() {
    let folder = Folder::new("named");
    let bundled = collection(&folder);
    let manifest = "[project]\ninclude_paths = [\"src\"]\n\n[dependencies]\nplant = { path = \"vendor/plant\" }\n";
    folder.write("project/trust-lsp.toml", manifest);
    let answer = install(
        &folder.0.join("project"),
        &bundled,
        json!([{ "name": "Plant", "folder": "plant" }]),
        json!({}),
    )
    .unwrap();
    assert_eq!(
        created(&answer),
        Vec::<String>::new(),
        "named whatever its case"
    );
    assert_eq!(answer["manifest"], json!(null));
}

#[test]
fn a_project_file_that_differs_from_the_bundle_stops_the_install() {
    // Overwriting it would throw away the user's own change to the library.
    let folder = Folder::new("differs");
    let bundled = collection(&folder);
    folder.write(
        "project/libraries/plant/src/pump.st",
        "FUNCTION_BLOCK Pump (* mine *) END_FUNCTION_BLOCK\n",
    );
    let project = folder.0.join("project");
    let error = install(
        &project,
        &bundled,
        json!([{ "name": "Plant", "folder": "plant" }]),
        json!({}),
    )
    .unwrap_err();
    assert!(
        error.ends_with("pump.st differs from the bundled library. Review that library update first; your version was preserved."),
        "{error}"
    );
    // An open editor's unsaved text counts, not the disk's.
    let open = json!({ "libraries/plant/src/pump.st": "FUNCTION_BLOCK Pump END_FUNCTION_BLOCK\n" });
    let answer = install(
        &project,
        &bundled,
        json!([{ "name": "Plant", "folder": "plant" }]),
        open,
    )
    .unwrap();
    assert!(
        !created(&answer).contains(&"libraries/plant/src/pump.st".to_owned()),
        "already there"
    );
    assert_eq!(
        answer["manifest"]["path"],
        json!("trust-lsp.toml"),
        "a project without a manifest gets one"
    );
}

#[test]
fn a_package_outside_the_collection_a_git_dependency_a_cycle_and_a_wrong_version_are_refused() {
    let folder = Folder::new("refused");
    let bundled = collection(&folder);
    let project = folder.0.join("project");
    std::fs::create_dir_all(&project).unwrap();
    let refusal = |libraries: Value| install(&project, &bundled, libraries, json!({})).unwrap_err();
    assert_eq!(
        refusal(json!([{ "name": "Bad", "folder": "../outside" }])),
        "Bundled library dependencies must stay inside the package collection."
    );
    folder.write(
        "extension/escape/trust-lsp.toml",
        "[package]\nversion=\"1.0.0\"\n[dependencies]\nX={path=\"../../elsewhere\"}\n",
    );
    assert_eq!(
        refusal(json!([{ "name": "Escape", "folder": "escape" }])),
        "A bundled dependency leaves the package collection."
    );
    folder.write(
        "extension/remote/trust-lsp.toml",
        "[package]\nversion=\"1.0.0\"\n[dependencies]\nX={git=\"https://example.com/x.git\"}\n",
    );
    assert_eq!(
        refusal(json!([{ "name": "Remote", "folder": "remote" }])),
        "A bundled dependency must name a local package."
    );
    folder.write(
        "extension/a/trust-lsp.toml",
        "[package]\nversion=\"1.0.0\"\n[dependencies]\nB={path=\"../b\"}\n",
    );
    folder.write(
        "extension/b/trust-lsp.toml",
        "[package]\nversion=\"1.0.0\"\n[dependencies]\nA={path=\"../a\"}\n",
    );
    assert_eq!(
        refusal(json!([{ "name": "A", "folder": "a" }])),
        "The bundled library contains a dependency cycle."
    );
    folder.write("extension/old/trust-lsp.toml", "[package]\nversion=\"1.0.0\"\n[dependencies]\nOSCAT={path=\"../oscat\",version=\"0.2.0\"}\n");
    assert_eq!(
        refusal(json!([{ "name": "Old", "folder": "old" }])),
        "The bundled OSCAT version does not match its contract."
    );
}
