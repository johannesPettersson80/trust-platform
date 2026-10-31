//! The examples gallery (specification 25 §3.1). Each test writes its gallery; the expected entries
//! follow from the files written. The extension's former `loadExamples` isolation cases live here.

use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;

static NEXT: AtomicUsize = AtomicUsize::new(0);

pub(super) struct Gallery(pub(super) PathBuf);

impl Gallery {
    fn new(manifest: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-examples-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("gallery");
        std::fs::write(path.join("manifest.json"), manifest).expect("manifest");
        Self(path)
    }

    fn write(&self, relative: &str, text: &str) {
        let path = self.0.join(relative);
        std::fs::create_dir_all(path.parent().expect("parent")).expect("folder");
        std::fs::write(path, text).expect("file");
    }

    fn catalog(&self, created: &[&str]) -> Result<Value, String> {
        catalog_answer(json!({ "root": self.0, "created": created }))
    }
}

impl Drop for Gallery {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// The gallery the extension packages: its build copies each entry of `examples/catalog.json` and
/// writes the catalog as `manifest.json` (`editors/vscode/scripts/build-examples.js`).
pub(super) fn packaged_gallery() -> Gallery {
    let examples = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let catalog = std::fs::read_to_string(examples.join("catalog.json")).expect("the catalog");
    let gallery = Gallery::new(&catalog);
    let entries: Vec<Value> = serde_json::from_str(&catalog).expect("the catalog's JSON");
    for entry in &entries {
        let path = entry["path"].as_str().expect("an entry's path");
        copy_folder(&examples.join(path), &gallery.0.join(path));
    }
    gallery
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("a folder");
    for item in std::fs::read_dir(from).expect("the example's folder") {
        let item = item.expect("a folder entry");
        let target = to.join(item.file_name());
        if item.file_type().expect("its type").is_dir() {
            copy_folder(&item.path(), &target);
        } else {
            std::fs::copy(item.path(), &target).expect("a copy");
        }
    }
}

fn entry(answer: &Value, id: &str) -> Value {
    answer["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .find(|entry| entry["id"] == id)
        .cloned()
        .expect("the entry")
}

/// The packaged first lesson, which holds its complete contract.
fn real_first_lesson() -> String {
    std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(
            "../../examples/control/01-feedback-foundations-and-temperature-control/01-cold-storage-loop-anatomy/lesson.json",
        ),
    )
    .expect("lesson.json")
}

const ISOLATION_MANIFEST: &str = r#"[
    {"id": "valid-starter", "title": "Valid starter", "description": "Runs", "path": "valid-starter", "hardware": "none"},
    {"id": "missing-lesson", "title": "Missing", "description": "Unavailable", "path": "missing-lesson", "hardware": "none", "tags": ["control"]},
    {"id": "invalid-json", "title": "Invalid JSON", "description": "Unavailable", "path": "invalid-json", "hardware": "none", "tags": ["control"]},
    {"id": "invalid-schema", "title": "Invalid schema", "description": "Unavailable", "path": "invalid-schema", "hardware": "none", "tags": ["control"]}
]"#;

#[test]
fn a_broken_lesson_is_unavailable_and_every_entry_stays() {
    // Spec 25 §3.1: a missing, unreadable or invalid lesson.json makes only that lesson unavailable,
    // with a concrete reason; the starter has no lesson state. Catches: one bad lesson failing the
    // gallery, or a broken lesson offered as runnable.
    let gallery = Gallery::new(ISOLATION_MANIFEST);
    gallery.write("invalid-json/lesson.json", "{not-json");
    gallery.write("invalid-schema/lesson.json", "{}");
    let answer = gallery.catalog(&[]).expect("the gallery loads");
    let ids: Vec<&str> = answer["entries"]
        .as_array()
        .expect("entries")
        .iter()
        .filter_map(|entry| entry["id"].as_str())
        .collect();
    assert_eq!(
        ids,
        [
            "valid-starter",
            "missing-lesson",
            "invalid-json",
            "invalid-schema"
        ]
    );
    let starter = entry(&answer, "valid-starter");
    assert_eq!(starter.get("lessonAvailability"), None);
    assert_eq!(starter.get("lesson"), None);
    assert_eq!(starter["copyable"], json!(true));
    for id in ["missing-lesson", "invalid-json"] {
        let broken = entry(&answer, id);
        assert_eq!(
            broken["lessonAvailability"]["kind"],
            json!("unavailable"),
            "{id}"
        );
        assert!(
            broken["lessonAvailability"]["reason"]
                .as_str()
                .is_some_and(|reason| reason.starts_with("Lesson metadata is unavailable: ")),
            "{id}: {broken}"
        );
        assert_eq!(broken["copyable"], json!(false), "{id}");
        assert_eq!(broken.get("lesson"), None, "{id}");
    }
    let invalid = entry(&answer, "invalid-schema");
    assert_eq!(
        invalid["lessonAvailability"],
        json!({ "kind": "unavailable", "reason": "Missing string 'level'." })
    );
    assert_eq!(invalid["copyable"], json!(false));
}

#[test]
fn a_complete_lesson_is_available_and_carries_its_contract() {
    let gallery = Gallery::new(
        r#"[{"id": "control-lesson-01", "title": "1. Control-loop elements", "description": "D", "path": "control/m/01", "hardware": "none", "tags": ["control", "simulator"]}]"#,
    );
    let lesson = real_first_lesson();
    gallery.write("control/m/01/lesson.json", &lesson);
    let answer = gallery.catalog(&[]).expect("the gallery loads");
    let first = entry(&answer, "control-lesson-01");
    assert_eq!(first["lessonAvailability"], json!({ "kind": "available" }));
    assert_eq!(first["copyable"], json!(true));
    assert_eq!(
        first["lesson"],
        serde_json::from_str::<Value>(&lesson).expect("JSON")
    );
    assert_eq!(first["tags"], json!(["control", "simulator"]));
}

#[test]
fn a_malformed_manifest_fails_the_gallery() {
    // Spec 25 §3.1: entry-level isolation never hides catalog corruption.
    let not_json = Gallery::new("not-json").catalog(&[]).expect_err("refused");
    assert!(
        not_json.starts_with("The examples manifest is not valid JSON: "),
        "{not_json}"
    );
    assert_eq!(
        Gallery::new("{}").catalog(&[]).expect_err("refused"),
        "examples manifest must be an array"
    );
    assert_eq!(
        Gallery::new("[1]").catalog(&[]).expect_err("refused"),
        "examples manifest entry 0 is not an object"
    );
    assert_eq!(
        Gallery::new(
            r#"[{"id": "a", "title": "", "description": "d", "path": "p", "hardware": "none"}]"#
        )
        .catalog(&[])
        .expect_err("refused"),
        "examples manifest entry 0 missing string 'title'"
    );
    let missing =
        catalog_answer(json!({ "root": "/nonexistent/trust-examples" })).expect_err("refused");
    assert!(
        missing.starts_with("Could not read the examples manifest: "),
        "{missing}"
    );
}

#[test]
fn progress_keeps_every_listed_lesson_and_drops_ids_the_gallery_no_longer_lists() {
    // Progress belongs to a stable lesson id: an unavailable lesson whose summary still orders the
    // course keeps it; a removed lesson and a starter (no lesson) drop. The stored order stays.
    // Catches: progress lost while a lesson is temporarily invalid, or kept for a removed lesson.
    let gallery = Gallery::new(
        r#"[
            {"id": "starter", "title": "S", "description": "d", "path": "starter", "hardware": "none"},
            {"id": "summary-only", "title": "2", "description": "d", "path": "control/m/02", "hardware": "none", "tags": ["control"]},
            {"id": "broken", "title": "3", "description": "d", "path": "control/m/03", "hardware": "none", "tags": ["control"]}
        ]"#,
    );
    gallery.write(
        "control/m/02/lesson.json",
        r#"{"schemaVersion": 2, "sequence": 2, "level": "Foundation", "equipment": "Room", "principalTechnique": "P", "prerequisites": []}"#,
    );
    gallery.write("control/m/03/lesson.json", "{");
    let answer = gallery
        .catalog(&["summary-only", "removed", "starter", "broken"])
        .expect("the gallery loads");
    assert_eq!(answer["createdLessonIds"], json!(["summary-only"]));
    let summary = entry(&answer, "summary-only");
    assert_eq!(summary["copyable"], json!(false));
    assert_eq!(summary["lesson"]["sequence"], json!(2));
}

#[test]
fn every_lesson_of_the_packaged_gallery_can_be_copied() {
    // The VS Code extension's packaged gallery: 101 entries, of which the 89 course lessons hold
    // their complete contracts. Catches: a shipped lesson the gallery would show as unavailable.
    let gallery = packaged_gallery();
    let answer = catalog_answer(json!({ "root": gallery.0 })).expect("the gallery loads");
    let entries = answer["entries"].as_array().expect("entries");
    let lessons: Vec<&Value> = entries
        .iter()
        .filter(|entry| entry.get("lessonAvailability").is_some())
        .collect();
    assert_eq!(lessons.len(), 89);
    let refused: Vec<String> = lessons
        .iter()
        .filter(|entry| entry["copyable"] != json!(true))
        .map(|entry| format!("{}: {}", entry["id"], entry["lessonAvailability"]["reason"]))
        .collect();
    assert_eq!(refused, Vec::<String>::new());
    assert!(entries.iter().all(|entry| entry["copyable"] == json!(true)));
}

#[test]
fn the_readme_action_reports_whether_it_changed_the_copy() {
    let gallery = Gallery::new(
        r#"[{"id": "a", "title": "A", "description": "d", "path": "control/m/01", "hardware": "none", "tags": ["control"]}]"#,
    );
    gallery.write("copy/README.md", "No links here.\n");
    let answer = copy_answer(json!({
        "action": "readme",
        "root": gallery.0,
        "entryPath": "control/m/01",
        "destination": gallery.0.join("copy"),
    }))
    .expect("answer");
    assert_eq!(answer, json!({ "ok": true, "changed": false }));
    let missing = copy_answer(json!({
        "action": "readme",
        "root": gallery.0,
        "entryPath": "control/m/01",
        "destination": gallery.0.join("nowhere"),
    }))
    .expect_err("no README");
    assert!(missing.starts_with("Could not read "), "{missing}");
}

#[test]
fn a_copy_takes_a_folder_name_by_the_rule_of_a_new_project() {
    // The extension's former `validateExampleFolderName` cases (specification 25 §3.1): empty, a path
    // marker, a separator, a character Windows refuses and a reserved device name are refused; a plain
    // name passes, trimmed. Catches: a copy into a folder outside the destination or one Windows
    // cannot open.
    for invalid in [
        "",
        ".",
        "..",
        "lesson/name",
        "lesson\\name",
        "lesson<1>",
        "CON",
        "nul.txt",
        "lesson.",
    ] {
        let answer = copy_answer(json!({ "action": "name", "name": invalid })).expect("answer");
        let problem = answer["problem"].as_str().unwrap_or_default();
        assert!(
            problem.starts_with("The project name ") && problem.ends_with('.'),
            "{invalid:?}: {answer}"
        );
    }
    let answer =
        copy_answer(json!({ "action": "name", "name": "  cold-storage-lesson " })).expect("answer");
    assert_eq!(answer["problem"], Value::Null);
    assert_eq!(answer["name"], json!("cold-storage-lesson"));
}
