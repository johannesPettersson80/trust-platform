//! The packaging check of the examples catalog (specification 25 §3.1; specification 38). The
//! messages are the extension build's former ones (`scripts/build-examples.js`).

use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};

static NEXT: AtomicUsize = AtomicUsize::new(0);

/// An examples folder with a `catalog.json`, removed when the test ends.
struct Examples(PathBuf);

impl Examples {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "trust-lsp-catalog-check-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&path);
        std::fs::create_dir_all(&path).expect("examples");
        Self(path)
    }

    fn folder(&self, relative: &str) {
        std::fs::create_dir_all(self.0.join(relative)).expect("folder");
    }

    fn lesson(&self, relative: &str, sequence: u32, prerequisites: &[&str]) {
        self.folder(relative);
        let lesson = serde_json::json!({
            "schemaVersion": 2,
            "level": "foundation",
            "equipment": "cold room",
            "principalTechnique": "on/off",
            "sequence": sequence,
            "prerequisites": prerequisites,
        });
        std::fs::write(
            self.0.join(relative).join("lesson.json"),
            lesson.to_string(),
        )
        .expect("lesson");
    }

    fn catalog(&self, entries: &Value) -> Result<usize, String> {
        std::fs::write(self.0.join("catalog.json"), entries.to_string()).expect("catalog");
        check_catalog(&self.0, "catalog.json")
    }
}

impl Drop for Examples {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn entry(id: &str, path: &str) -> Value {
    serde_json::json!({
        "id": id, "title": id, "description": "d", "path": path, "hardware": "none",
    })
}

fn lesson_entry(id: &str, path: &str) -> Value {
    let mut entry = entry(id, path);
    entry["tags"] = serde_json::json!(["control"]);
    entry
}

#[test]
fn a_catalog_of_a_starter_and_an_ordered_course_passes() {
    let examples = Examples::new();
    examples.folder("starter");
    examples.lesson("control/one", 1, &[]);
    examples.lesson("control/two", 2, &["control-one"]);
    let checked = examples.catalog(&serde_json::json!([
        entry("starter", "starter"),
        lesson_entry("control-one", "control/one"),
        lesson_entry("control-two", "control/two"),
    ]));
    assert_eq!(checked, Ok(3));
}

#[test]
fn an_entry_without_a_field_an_unsafe_path_or_a_folder_is_refused() {
    // Catches: a package that copies outside the examples folder or ships an entry the gallery
    // cannot show.
    let examples = Examples::new();
    examples.folder("starter");
    let mut untitled = entry("starter", "starter");
    untitled["title"] = serde_json::json!("  ");
    assert_eq!(
        examples.catalog(&serde_json::json!([untitled])),
        Err("catalog entry 0 is missing 'title'".to_owned())
    );
    assert_eq!(
        examples.catalog(&serde_json::json!([entry("away", "starter/../../away")])),
        Err("catalog entry 0 has an unsafe path 'starter/../../away'".to_owned())
    );
    assert_eq!(
        examples.catalog(&serde_json::json!([entry("abs", "/etc")])),
        Err("catalog entry 0 has an unsafe path '/etc'".to_owned())
    );
    assert_eq!(
        examples.catalog(&serde_json::json!([entry("missing", "missing")])),
        Err("catalog entry 0 is not a directory: missing".to_owned())
    );
}

#[test]
fn a_repeated_id_or_path_is_refused() {
    let examples = Examples::new();
    examples.folder("starter");
    examples.folder("other");
    assert_eq!(
        examples.catalog(&serde_json::json!([
            entry("starter", "starter"),
            entry("starter", "other"),
        ])),
        Err("catalog entry 1 duplicates an id or path".to_owned())
    );
    assert_eq!(
        examples.catalog(&serde_json::json!([
            entry("starter", "starter"),
            entry("again", "./starter"),
        ])),
        Err("catalog entry 1 duplicates an id or path".to_owned()),
        "`./starter` is the folder `starter`"
    );
}

#[test]
fn a_control_lesson_needs_its_lesson_file_and_a_valid_contract() {
    // Catches: a lesson packaged that the gallery could only show as unavailable.
    let examples = Examples::new();
    examples.folder("control/bare");
    assert_eq!(
        examples.catalog(&serde_json::json!([lesson_entry(
            "control-bare",
            "control/bare"
        )])),
        Err("control lesson 'control-bare' is missing lesson.json".to_owned())
    );
    std::fs::write(
        examples.0.join("control/bare/lesson.json"),
        r#"{"schemaVersion": 2, "level": "x", "equipment": "y", "principalTechnique": "z", "sequence": 0, "prerequisites": []}"#,
    )
    .expect("lesson");
    assert_eq!(
        examples.catalog(&serde_json::json!([lesson_entry(
            "control-bare",
            "control/bare"
        )])),
        Err("control lesson 'control-bare' has invalid sequence".to_owned())
    );
}

#[test]
fn an_id_starting_with_control_is_a_lesson_even_without_the_tag() {
    // The build's rule: the `control-` id prefix or the `control` tag. Catches: an untagged lesson
    // packaged without its check.
    let examples = Examples::new();
    examples.folder("control/untagged");
    assert_eq!(
        examples.catalog(&serde_json::json!([entry(
            "control-untagged",
            "control/untagged"
        )])),
        Err("control lesson 'control-untagged' is missing lesson.json".to_owned())
    );
}

#[test]
fn a_broken_course_order_is_refused() {
    // Sequences 1 and 3: lesson 2 is missing. Catches: a course with a hole in its order.
    let examples = Examples::new();
    examples.lesson("control/one", 1, &[]);
    examples.lesson("control/three", 3, &["control-one"]);
    assert_eq!(
        examples.catalog(&serde_json::json!([
            lesson_entry("control-one", "control/one"),
            lesson_entry("control-three", "control/three"),
        ])),
        Err(
            "control curriculum is invalid: control-three: sequence gap, expected 2, found 3"
                .to_owned()
        )
    );
}

#[test]
fn the_repository_catalog_passes_the_packaging_check() {
    // The check the extension's build runs over `examples/catalog.json`.
    let examples = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../examples");
    let checked = check_catalog(&examples, "catalog.json").expect("the shipped catalog");
    assert!(checked > 89, "{checked} entries");
}
