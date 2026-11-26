//! The examples gallery (specification 25 §3.1; specification 38).
//!
//! - `trust-lsp.examplesCatalog` `{ root, created? }`: the gallery's entries from the packaged
//!   `manifest.json` in `root`, each control lesson with its availability, whether each entry can be
//!   copied, what each card says of its packaged default (`execution`, `hardwareLabel` and
//!   `hardwareRequired`, specification 25 §3.1), each entry's program file, which a copied starter
//!   opens (`program`, §3.2), and which of the `created` lesson ids the learner's progress keeps. A manifest that
//!   cannot be read, or whose entries miss their fields, fails the gallery; one lesson whose
//!   `lesson.json` is missing, unreadable or invalid only makes that lesson unavailable. The
//!   repository's catalog, which the extension's build copies, passes the stricter packaging check
//!   of the tests (`catalog_check`): there any broken entry or lesson, or a broken course order,
//!   refuses the whole catalog.
//! - `trust-lsp.copyExample` `{ action: "readme", root, entryPath, destination }`: the README of a
//!   copied lesson, whose links to sibling lessons name how to open them from the gallery.
//! - `trust-lsp.copyExample` `{ action: "name", name }`: whether a copy's folder name is one plain
//!   folder name on every platform, by the rule of a new project's name
//!   (`trust_runtime::new_project::folder_name_problem`).

#[cfg(test)]
mod catalog_check;
mod lesson;
mod readme;

use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::{json, Map, Value};
use trust_runtime::native::rust_pous;
use trust_runtime::new_project::ProjectStart;

use super::decode;

#[derive(Deserialize)]
struct CatalogArgs {
    /// The extension's `media/examples` folder.
    root: PathBuf,
    /// The lesson ids the learner created projects for, as the extension stored them.
    #[serde(default)]
    created: Vec<String>,
}

#[derive(Deserialize)]
#[serde(tag = "action", rename_all = "lowercase")]
enum CopyArgs {
    /// Adapts the README of a copied lesson.
    Readme {
        root: PathBuf,
        #[serde(rename = "entryPath")]
        entry_path: String,
        destination: PathBuf,
    },
    /// Checks the folder name of a copy.
    Name { name: String },
}

/// One entry of the manifest.
struct ManifestEntry {
    id: String,
    title: String,
    description: String,
    path: String,
    hardware: String,
    /// The tags, as the manifest gives them, when it gives a list.
    tags: Option<Vec<Value>>,
}

impl ManifestEntry {
    fn is_control_lesson(&self) -> bool {
        self.tags
            .as_ref()
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some("control")))
    }
}

/// `{ ok, entries, createdLessonIds }`.
pub(super) fn catalog_answer(argument: Value) -> Result<Value, String> {
    let args: CatalogArgs = decode(argument)?;
    let manifest = read_manifest(&args.root)?;
    let mut entries = Vec::with_capacity(manifest.len());
    let mut lesson_ids = Vec::new();
    for entry in &manifest {
        let answer = entry_answer(&args.root, entry);
        if answer.contains_key("lesson") {
            lesson_ids.push(entry.id.as_str());
        }
        entries.push(Value::Object(answer));
    }
    Ok(json!({
        "ok": true,
        "entries": entries,
        "createdLessonIds": retained_progress(&args.created, &lesson_ids),
    }))
}

/// `readme`: `{ ok, changed }`. `name`: `{ ok, problem, name }`, the problem as a sentence or `null`,
/// and the trimmed name the copy takes.
pub(super) fn copy_answer(argument: Value) -> Result<Value, String> {
    match decode::<CopyArgs>(argument)? {
        CopyArgs::Readme {
            root,
            entry_path,
            destination,
        } => {
            let changed = readme::adapt_copied_readme(&root, &entry_path, &destination)?;
            Ok(json!({ "ok": true, "changed": changed }))
        }
        CopyArgs::Name { name } => {
            let problem = trust_runtime::new_project::folder_name_problem(&name).map(as_sentence);
            Ok(json!({ "ok": true, "problem": problem, "name": name.trim() }))
        }
    }
}

/// `text` with a capital first letter and a final dot, as an input box shows a problem.
fn as_sentence(text: &str) -> String {
    let mut chars = text.chars();
    match chars.next() {
        Some(first) => format!("{}{}.", first.to_uppercase(), chars.as_str()),
        None => String::new(),
    }
}

/// The entries of `<root>/manifest.json`.
fn read_manifest(root: &Path) -> Result<Vec<ManifestEntry>, String> {
    let text = std::fs::read_to_string(root.join("manifest.json"))
        .map_err(|error| format!("Could not read the examples manifest: {error}"))?;
    let raw: Value = serde_json::from_str(&text)
        .map_err(|error| format!("The examples manifest is not valid JSON: {error}"))?;
    parse_manifest(&raw)
}

/// The manifest's entries: a list of objects, each with the texts `id`, `title`, `description`,
/// `path` and `hardware`, and optionally a list of `tags`.
fn parse_manifest(raw: &Value) -> Result<Vec<ManifestEntry>, String> {
    let items = raw
        .as_array()
        .ok_or_else(|| "examples manifest must be an array".to_owned())?;
    items
        .iter()
        .enumerate()
        .map(|(index, item)| {
            let entry = item
                .as_object()
                .ok_or_else(|| format!("examples manifest entry {index} is not an object"))?;
            let text = |field: &str| {
                entry
                    .get(field)
                    .and_then(Value::as_str)
                    .filter(|text| !text.is_empty())
                    .map(str::to_owned)
                    .ok_or_else(|| {
                        format!("examples manifest entry {index} missing string '{field}'")
                    })
            };
            Ok(ManifestEntry {
                id: text("id")?,
                title: text("title")?,
                description: text("description")?,
                path: text("path")?,
                hardware: text("hardware")?,
                tags: entry.get("tags").and_then(Value::as_array).cloned(),
            })
        })
        .collect()
}

/// One gallery entry. A control lesson carries its availability, and its lesson when the contract
/// or at least the summary can be read. A starter is always copyable; a lesson only when available.
fn entry_answer(root: &Path, entry: &ManifestEntry) -> Map<String, Value> {
    let mut answer = Map::new();
    answer.insert("id".to_owned(), json!(entry.id));
    answer.insert("title".to_owned(), json!(entry.title));
    answer.insert("description".to_owned(), json!(entry.description));
    answer.insert("path".to_owned(), json!(entry.path));
    answer.insert(
        "program".to_owned(),
        json!(program_file(&root.join(&entry.path))),
    );
    answer.insert("hardware".to_owned(), json!(entry.hardware));
    if let Some(tags) = &entry.tags {
        answer.insert("tags".to_owned(), json!(tags));
    }
    let badges = card_badges(root, entry);
    if let Some(execution) = badges.execution {
        answer.insert("execution".to_owned(), json!(execution));
    }
    answer.insert("hardwareLabel".to_owned(), json!(badges.hardware_label));
    answer.insert(
        "hardwareRequired".to_owned(),
        json!(badges.hardware_required),
    );
    if !entry.is_control_lesson() {
        answer.insert("copyable".to_owned(), json!(true));
        return answer;
    }
    let availability = match read_lesson(root, &entry.path) {
        Ok(raw) => lesson::availability(&raw, &entry.id),
        Err(detail) => lesson::Availability {
            available: false,
            reason: Some(format!("Lesson metadata is unavailable: {detail}")),
            lesson: None,
        },
    };
    if let Some(lesson) = availability.lesson {
        answer.insert("lesson".to_owned(), lesson);
    }
    let mut state = Map::new();
    if availability.available {
        state.insert("kind".to_owned(), json!("available"));
    } else {
        state.insert("kind".to_owned(), json!("unavailable"));
        state.insert("reason".to_owned(), json!(availability.reason));
    }
    answer.insert("lessonAvailability".to_owned(), Value::Object(state));
    answer.insert("copyable".to_owned(), json!(availability.available));
    answer
}

/// The program file of the project in `folder`, relative to it: the file a copied starter opens
/// (specification 25 §3.2), as Create Project opens a new project's (`ProjectStart::focus`). A
/// Rust-first project, whose `trust.toml` makes the project's own folder its crate (specification 48
/// RU-65), has its program in the crate, `src/lib.rs`. Every other project has it in `src/Main.st`,
/// which calls the crate of a project with Rust in `rust/`.
fn program_file(folder: &Path) -> &'static str {
    let rust_first = rust_pous::layout(folder).is_ok_and(|layout| layout.crate_folder == ".");
    if rust_first {
        ProjectStart::RustFirst.focus()
    } else {
        ProjectStart::StructuredText.focus()
    }
}

/// What a card says of an example's packaged default (specification 25 §3.1).
#[derive(Debug, PartialEq)]
struct CardBadges {
    /// `Runs in simulator` for an example that is not a course lesson and whose packaged `io.toml`
    /// uses the simulated driver.
    execution: Option<&'static str>,
    /// The hardware badge: optional for an example that runs in the simulator, else required.
    hardware_label: String,
    /// Whether the packaged default needs its hardware, which the badge shows as a warning.
    hardware_required: bool,
}

/// The badges of `entry`'s card. Its packaged `io.toml` decides whether it runs in the simulator;
/// its catalog hardware names the hardware (`none`, `twincat`, `raspberrypi` or another name).
fn card_badges(root: &Path, entry: &ManifestEntry) -> CardBadges {
    let hardware = match entry.hardware.as_str() {
        "none" => None,
        "twincat" => Some("TwinCAT".to_owned()),
        "raspberrypi" => Some("Raspberry Pi".to_owned()),
        other => Some(other.to_owned()),
    };
    let simulated = !entry.is_control_lesson() && ships_simulated_io(&root.join(&entry.path));
    match (simulated, hardware) {
        (true, None) => CardBadges {
            execution: Some("Runs in simulator"),
            hardware_label: "No hardware needed".to_owned(),
            hardware_required: false,
        },
        (true, Some(name)) => CardBadges {
            execution: Some("Runs in simulator"),
            hardware_label: format!("Hardware optional: {name}"),
            hardware_required: false,
        },
        (false, None) => CardBadges {
            execution: None,
            hardware_label: "No hardware".to_owned(),
            hardware_required: false,
        },
        (false, Some(name)) => CardBadges {
            execution: None,
            hardware_label: format!("Requires {name}"),
            hardware_required: true,
        },
    }
}

/// Whether the example in `folder` ships the simulated I/O driver (`[io] driver = "simulated"` in
/// its `io.toml`); an `io.toml` that is missing or unreadable is no simulated default.
fn ships_simulated_io(folder: &Path) -> bool {
    std::fs::read_to_string(folder.join("io.toml"))
        .ok()
        .and_then(|text| text.parse::<toml::Table>().ok())
        .and_then(|table| {
            table
                .get("io")
                .and_then(|io| io.get("driver"))
                .and_then(toml::Value::as_str)
                .map(|driver| driver == "simulated")
        })
        .unwrap_or(false)
}

/// The `lesson.json` of the lesson at gallery path `path`.
fn read_lesson(root: &Path, path: &str) -> Result<Value, String> {
    let text = std::fs::read_to_string(root.join(path).join("lesson.json"))
        .map_err(|error| error.to_string())?;
    serde_json::from_str(&text).map_err(|error| error.to_string())
}

/// The learner's progress belongs to a lesson's stable id, not to its availability: a lesson that is
/// unavailable for now keeps its progress while the gallery still lists it; only ids the gallery no
/// longer lists are dropped. The order of `created` stays.
fn retained_progress<'a>(created: &'a [String], lesson_ids: &[&str]) -> Vec<&'a str> {
    created
        .iter()
        .map(String::as_str)
        .filter(|id| lesson_ids.contains(id))
        .collect()
}

#[cfg(test)]
#[path = "examples_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "examples/curriculum_tests.rs"]
mod curriculum_tests;
