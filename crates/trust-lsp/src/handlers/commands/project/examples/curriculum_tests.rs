//! The control course as the repository ships it (specification 38; the teaching specification's
//! §10.4 existing-source map). These are the extension's former `control-course-navigation`
//! assertions: the catalog's lessons hold their contracts, follow the source map in teaching order,
//! and form a curriculum whose sequences run 1, 2, 3, … and whose prerequisites come earlier.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use super::catalog_check::{validate_curriculum, CourseLesson};
use super::lesson::{availability, parse_contract, Lesson};
use super::readme::normalize_posix;

fn repository() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn read_json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("file")).expect("JSON")
}

/// The catalog's control lessons, in catalog order: (id, path, title, description).
fn control_entries() -> Vec<(String, String, String, String)> {
    let catalog = read_json(&repository().join("examples/catalog.json"));
    catalog
        .as_array()
        .expect("catalog list")
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("control/"))
        })
        .map(|entry| {
            let text = |field: &str| entry[field].as_str().unwrap_or_default().to_owned();
            (text("id"), text("path"), text("title"), text("description"))
        })
        .collect()
}

fn lesson(id: &str, sequence: f64, prerequisites: &[&str]) -> CourseLesson {
    CourseLesson {
        id: id.to_owned(),
        sequence,
        prerequisites: prerequisites
            .iter()
            .map(|item| (*item).to_owned())
            .collect(),
    }
}

#[test]
fn the_curriculum_rule_names_gaps_repeats_and_late_prerequisites() {
    // Catches: a course order that skips or repeats a lesson, or asks for a later lesson first.
    assert_eq!(
        validate_curriculum(&[lesson("a", 1.0, &[]), lesson("b", 2.0, &["a"])]),
        Ok(())
    );
    assert!(
        validate_curriculum(&[lesson("a", 1.0, &[]), lesson("b", 3.0, &[])])
            .is_err_and(|error| error.contains("sequence gap"))
    );
    assert!(
        validate_curriculum(&[lesson("a", 1.0, &[]), lesson("b", 1.0, &[])])
            .is_err_and(|error| error.contains("duplicate sequence"))
    );
    assert!(
        validate_curriculum(&[lesson("a", 1.0, &["b"]), lesson("b", 2.0, &[])])
            .is_err_and(|error| error.contains("forward prerequisite"))
    );
    assert!(validate_curriculum(&[lesson("a", 1.0, &["zz"])])
        .is_err_and(|error| error.contains("unknown prerequisite")));
}

#[test]
fn the_catalog_lessons_hold_their_contracts_and_form_the_course() {
    // Every control lesson of the catalog is available, and the catalog's order is the course.
    // Catches: a shipped lesson the gallery would refuse, or a broken course order.
    let examples = repository().join("examples");
    let mut lessons = Vec::new();
    for (id, path, _, _) in control_entries() {
        let raw = read_json(&examples.join(&path).join("lesson.json"));
        let found = availability(&raw, &id);
        assert!(found.available, "{id}: {:?}", found.reason);
        lessons.push(CourseLesson::of(&raw));
    }
    assert_eq!(validate_curriculum(&lessons), Ok(()));
}

#[test]
fn the_manual_pi_lesson_follows_its_route() {
    let raw = read_json(
        &repository()
            .join("examples/control/06-pid-tuning-methods/28-manual-pi-tuning/lesson.json"),
    );
    let Ok(Lesson::Contract(lesson)) = parse_contract(&raw, "control-lesson-28") else {
        panic!("the manual PI lesson holds its contract");
    };
    assert_eq!(
        lesson["workflowStages"],
        serde_json::json!(["Monitor", "Tune", "Test", "Validate"])
    );
}

/// One row of the source map: `| <n>. <title> | [<name>](<path>/README.md) |`.
struct SourceRow {
    sequence: u64,
    title: String,
    /// The lesson folder, relative to `examples/`.
    project: String,
}

/// The rows of the teaching specification's §10.4 existing-source map.
fn source_map(specification: &str) -> Vec<SourceRow> {
    let map = specification
        .split("### 10.4 Existing-source map")
        .nth(1)
        .expect("the normative existing-source map must exist");
    map.lines().filter_map(source_row).collect()
}

fn source_row(line: &str) -> Option<SourceRow> {
    let rest = line.strip_prefix("| ")?;
    let digits_end = rest.find(|ch: char| !ch.is_ascii_digit())?;
    let sequence: u64 = rest[..digits_end].parse().ok()?;
    let rest = rest[digits_end..].strip_prefix(". ")?;
    let (title, link) = rest.split_once(" | [")?;
    let (_, link) = link.split_once("](")?;
    let target = link.strip_suffix(") |")?;
    if target.contains(')') || !target.ends_with("/README.md") {
        return None;
    }
    let resolved = normalize_posix(&format!("docs/specs/{target}"));
    let project = resolved
        .strip_prefix("examples/")?
        .strip_suffix("/README.md")?
        .to_owned();
    Some(SourceRow {
        sequence,
        title: title.to_owned(),
        project,
    })
}

/// The outcome column of the specification's lesson table: `| **<n>. …** | <outcome> |`, by number.
fn outcomes(specification: &str) -> HashMap<u64, String> {
    specification
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("| **")?;
            let digits_end = rest.find(|ch: char| !ch.is_ascii_digit())?;
            let sequence: u64 = rest[..digits_end].parse().ok()?;
            let rest = rest[digits_end..].strip_prefix(". ")?;
            let (_, after) = rest.split_once("** | ")?;
            let (outcome, _) = after.split_once(" |")?;
            Some((sequence, outcome.to_owned()))
        })
        .collect()
}

#[test]
fn the_catalog_follows_the_normative_89_lesson_source_map() {
    // The gallery exposes the accepted source projects in teaching order, without historical extras:
    // ids, sequences, titles, descriptions, README headings, next lessons and prerequisites agree.
    let root = repository();
    let specification =
        std::fs::read_to_string(root.join("docs/specs/control-course-teaching-specification.md"))
            .expect("teaching specification");
    let rows = source_map(&specification);
    assert_eq!(rows.len(), 89);
    let entries = control_entries();
    assert_eq!(
        entries
            .iter()
            .map(|(_, path, _, _)| path.as_str())
            .collect::<Vec<_>>(),
        rows.iter()
            .map(|row| row.project.as_str())
            .collect::<Vec<_>>(),
        "the gallery must expose the accepted source projects in teaching order"
    );
    let outcomes = outcomes(&specification);
    let mut lessons = Vec::new();
    for (index, row) in rows.iter().enumerate() {
        let folder = root.join("examples").join(&row.project);
        let lesson = read_json(&folder.join("lesson.json"));
        let (id, _, title, description) = &entries[index];
        assert_eq!(lesson["id"].as_str(), Some(id.as_str()));
        assert_eq!(lesson["sequence"].as_u64(), Some(row.sequence));
        assert_eq!(lesson["title"].as_str(), Some(row.title.as_str()));
        assert_eq!(title, &format!("{}. {}", row.sequence, row.title));
        assert_eq!(Some(description), outcomes.get(&row.sequence));
        let readme = std::fs::read_to_string(folder.join("README.md")).expect("README");
        assert_eq!(
            readme.split('\n').next(),
            Some(format!("# {}. {}", row.sequence, row.title).as_str())
        );
        let next = entries.get(index + 1).map(|(id, _, _, _)| id.as_str());
        assert_eq!(lesson["nextLesson"].as_str(), next, "{id}");
        let prerequisites = &lesson["prerequisites"];
        if index > 0 {
            let previous = entries[index - 1].0.as_str();
            assert!(
                prerequisites
                    .as_array()
                    .is_some_and(|items| items.iter().any(|item| item == previous)),
                "{id}: the predecessor must be a prerequisite"
            );
        } else {
            assert_eq!(prerequisites, &serde_json::json!([]));
        }
        lessons.push(CourseLesson::of(&lesson));
    }
    assert_eq!(validate_curriculum(&lessons), Ok(()));
}
