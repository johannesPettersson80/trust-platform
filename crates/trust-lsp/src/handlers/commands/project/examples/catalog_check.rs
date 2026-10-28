//! The packaging check of the examples catalog (specification 25 §3.1; specification 38). The
//! repository's `examples/catalog.json`, which the extension's build copies into its package, must
//! pass it: no entry misses its fields, leaves the examples folder, repeats an id or a path, or
//! names no folder; every control lesson has a `lesson.json` that holds its contract; and the
//! course's order is whole. The gallery itself keeps every entry and marks a broken lesson
//! unavailable; this check, which `cargo test` runs over the repository's catalog in CI and in the
//! release guard, keeps such a lesson out of a package.

use std::collections::{HashMap, HashSet};
use std::path::{Component, Path, PathBuf};

use serde_json::Value;

use super::lesson::{parse_contract, Lesson};

/// A lesson as the course orders it.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct CourseLesson {
    pub(super) id: String,
    pub(super) sequence: f64,
    pub(super) prerequisites: Vec<String>,
}

impl CourseLesson {
    /// The course fields of a lesson file.
    pub(super) fn of(lesson: &Value) -> Self {
        Self {
            id: lesson["id"].as_str().unwrap_or_default().to_owned(),
            sequence: lesson["sequence"].as_f64().unwrap_or(f64::NAN),
            prerequisites: lesson["prerequisites"]
                .as_array()
                .map(|items| {
                    items
                        .iter()
                        .filter_map(|item| item.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        }
    }
}

/// The course rule: the n-th lesson has sequence n, no sequence repeats, and every prerequisite is
/// a lesson of the course with a smaller sequence. The first broken rule is named.
pub(super) fn validate_curriculum(lessons: &[CourseLesson]) -> Result<(), String> {
    let by_id: HashMap<&str, &CourseLesson> = lessons
        .iter()
        .map(|lesson| (lesson.id.as_str(), lesson))
        .collect();
    let mut seen: Vec<f64> = Vec::new();
    for (index, lesson) in lessons.iter().enumerate() {
        if seen.contains(&lesson.sequence) {
            return Err(format!(
                "{}: duplicate sequence {}",
                lesson.id, lesson.sequence
            ));
        }
        seen.push(lesson.sequence);
        let expected = f64::from(u32::try_from(index + 1).unwrap_or(u32::MAX));
        if lesson.sequence != expected {
            return Err(format!(
                "{}: sequence gap, expected {expected}, found {}",
                lesson.id, lesson.sequence
            ));
        }
        for prerequisite in &lesson.prerequisites {
            let Some(dependency) = by_id.get(prerequisite.as_str()) else {
                return Err(format!(
                    "{}: unknown prerequisite {prerequisite}",
                    lesson.id
                ));
            };
            if dependency.sequence >= lesson.sequence {
                return Err(format!(
                    "{}: forward prerequisite {prerequisite}",
                    lesson.id
                ));
            }
        }
    }
    Ok(())
}

/// Checks the catalog `<root>/<manifest>` for packaging; the number of its entries when it passes.
pub(super) fn check_catalog(root: &Path, manifest: &str) -> Result<usize, String> {
    let text = std::fs::read_to_string(root.join(manifest))
        .map_err(|error| format!("Could not read {manifest}: {error}"))?;
    let raw: Value = serde_json::from_str(&text)
        .map_err(|error| format!("{manifest} is not valid JSON: {error}"))?;
    let entries = raw
        .as_array()
        .ok_or_else(|| format!("{manifest} must contain an array"))?;
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    let mut lessons = Vec::new();
    for (index, entry) in entries.iter().enumerate() {
        for field in ["id", "title", "description", "path", "hardware"] {
            if entry[field]
                .as_str()
                .is_none_or(|text| text.trim().is_empty())
            {
                return Err(format!("catalog entry {index} is missing '{field}'"));
            }
        }
        let id = entry["id"].as_str().unwrap_or_default();
        let path = entry["path"].as_str().unwrap_or_default();
        let relative = safe_relative(path)
            .ok_or_else(|| format!("catalog entry {index} has an unsafe path '{path}'"))?;
        if !ids.insert(id.to_owned()) || !paths.insert(relative.clone()) {
            return Err(format!("catalog entry {index} duplicates an id or path"));
        }
        let folder = root.join(&relative);
        if !folder.is_dir() {
            return Err(format!("catalog entry {index} is not a directory: {path}"));
        }
        if !is_control_lesson(entry, id) {
            continue;
        }
        let lesson_path = folder.join("lesson.json");
        if !lesson_path.is_file() {
            return Err(format!("control lesson '{id}' is missing lesson.json"));
        }
        let lesson: Value = std::fs::read_to_string(&lesson_path)
            .map_err(|error| format!("control lesson '{id}' cannot be read: {error}"))
            .and_then(|text| {
                serde_json::from_str(&text)
                    .map_err(|error| format!("control lesson '{id}' is not valid JSON: {error}"))
            })?;
        let course_fields = match parse_contract(&lesson, id)? {
            Lesson::Summary(fields) | Lesson::Contract(fields) => fields,
        };
        lessons.push(CourseLesson {
            id: id.to_owned(),
            ..CourseLesson::of(&course_fields)
        });
    }
    validate_curriculum(&lessons)
        .map_err(|problem| format!("control curriculum is invalid: {problem}"))?;
    Ok(entries.len())
}

/// A control lesson: an id that starts with `control-`, or the tag `control`.
fn is_control_lesson(entry: &Value, id: &str) -> bool {
    id.starts_with("control-")
        || entry["tags"]
            .as_array()
            .is_some_and(|tags| tags.iter().any(|tag| tag.as_str() == Some("control")))
}

/// `path` without `.` parts, when it stays inside the examples folder: not absolute, and no `..`
/// that leaves it.
fn safe_relative(path: &str) -> Option<PathBuf> {
    let candidate = Path::new(path);
    if candidate.is_absolute() || path.starts_with('/') || path.starts_with('\\') {
        return None;
    }
    let mut parts: Vec<&std::ffi::OsStr> = Vec::new();
    for component in candidate.components() {
        match component {
            Component::Normal(part) => parts.push(part),
            Component::CurDir => {}
            Component::ParentDir => {
                parts.pop()?;
            }
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    (!parts.is_empty()).then(|| parts.iter().collect())
}

#[cfg(test)]
#[path = "catalog_check_tests.rs"]
mod tests;
