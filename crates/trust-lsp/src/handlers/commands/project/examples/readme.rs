//! The README of a copied control lesson (specification 25 §3.1, §3.2). A lesson's README links its
//! sibling lessons by relative paths, which a copied project does not have. In the copy, each link
//! that resolves to a lesson of the gallery's manifest keeps its text and is followed by how to
//! open that lesson from the gallery; external links, anchors, images and links to folders the
//! manifest does not name stay as written.

use std::path::Path;

use serde_json::Value;

/// One Markdown link `[text](target)`: its byte range in the README, its text and its target.
struct Link<'a> {
    start: usize,
    end: usize,
    text: &'a str,
    target: &'a str,
}

/// Rewrites the README of the copied lesson in `destination`, whose gallery path is `entry_path`,
/// with the manifest and lessons of the gallery in `root`. Answers whether it changed the file.
pub(super) fn adapt_copied_readme(
    root: &Path,
    entry_path: &str,
    destination: &Path,
) -> Result<bool, String> {
    let readme = destination.join("README.md");
    let source = std::fs::read_to_string(&readme)
        .map_err(|error| format!("Could not read {}: {error}", readme.display()))?;
    let links = links(&source);
    if links.is_empty() {
        return Ok(false);
    }
    let manifest_path = root.join("manifest.json");
    let manifest: Value = std::fs::read_to_string(&manifest_path)
        .map_err(|error| format!("Could not read the examples manifest: {error}"))
        .and_then(|text| {
            serde_json::from_str(&text)
                .map_err(|error| format!("The examples manifest is not valid JSON: {error}"))
        })?;
    let entries = manifest
        .as_array()
        .ok_or_else(|| "examples manifest must be an array".to_owned())?;
    let mut output = String::with_capacity(source.len());
    let mut cursor = 0;
    for link in links {
        let Some(target_path) = relative_lesson_path(entry_path, link.target) else {
            continue;
        };
        let Some(target) = entries
            .iter()
            .filter_map(|entry| entry.get("path").and_then(Value::as_str))
            .find(|path| *path == target_path)
        else {
            continue;
        };
        let Some(number) = lesson_number(root, target, link.text) else {
            continue;
        };
        output.push_str(&source[cursor..link.start]);
        output.push_str(link.text);
        output.push_str(&format!(
            " (to open it, use truST > Start from example and select Lesson {number})"
        ));
        cursor = link.end;
    }
    if cursor == 0 {
        return Ok(false);
    }
    output.push_str(&source[cursor..]);
    std::fs::write(&readme, output)
        .map_err(|error| format!("Could not write {}: {error}", readme.display()))?;
    Ok(true)
}

/// The links of `source` in order: `[text](target)` not preceded by `!` (an image), where the
/// text holds no `]` and the target no `)`, and neither spans a line.
fn links(source: &str) -> Vec<Link<'_>> {
    let bytes = source.as_bytes();
    let mut found = Vec::new();
    let mut index = 0;
    while let Some(offset) = source[index..].find('[') {
        let start = index + offset;
        index = start + 1;
        if start > 0 && bytes[start - 1] == b'!' {
            continue;
        }
        let Some(link) = link_at(source, start) else {
            continue;
        };
        index = link.end;
        found.push(link);
    }
    found
}

/// The link that starts with the `[` at `start`, if one does.
fn link_at(source: &str, start: usize) -> Option<Link<'_>> {
    let text_start = start + 1;
    let text_end = text_start + source[text_start..].find([']', '\n'])?;
    if text_end == text_start || !source[text_end..].starts_with("](") {
        return None;
    }
    let target_start = text_end + 2;
    let target_end = target_start + source[target_start..].find([')', '\n'])?;
    if target_end == target_start || !source[target_end..].starts_with(')') {
        return None;
    }
    Some(Link {
        start,
        end: target_end + 1,
        text: &source[text_start..text_end],
        target: &source[target_start..target_end],
    })
}

/// The gallery path a link of the lesson at `entry_path` points to, when it is a relative path into
/// the control course: the link resolved against the lesson's path, without a final `/README.md`.
fn relative_lesson_path(entry_path: &str, target: &str) -> Option<String> {
    if target.is_empty() || target.starts_with('#') || target.starts_with('/') || has_scheme(target)
    {
        return None;
    }
    let target = target.strip_prefix("./").unwrap_or(target);
    // A POSIX join leaves out an empty part, so `./` alone names the lesson's own folder.
    let joined = if target.is_empty() {
        normalize_posix(entry_path)
    } else {
        normalize_posix(&format!("{entry_path}/{target}"))
    };
    let resolved = joined.strip_suffix("/README.md").unwrap_or(&joined);
    resolved
        .starts_with("control/")
        .then(|| resolved.to_owned())
}

/// Whether `target` begins with a URI scheme such as `https:` or `mailto:`.
fn has_scheme(target: &str) -> bool {
    let Some(colon) = target.find(':') else {
        return false;
    };
    let scheme = &target[..colon];
    let mut chars = scheme.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '+' | '.' | '-'))
}

/// `path` with `.` and `..` resolved and repeated slashes merged, as POSIX normalizes a relative
/// path; a final slash stays.
pub(super) fn normalize_posix(path: &str) -> String {
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|last| *last != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            part => parts.push(part),
        }
    }
    let mut normalized = parts.join("/");
    if normalized.is_empty() {
        normalized.push('.');
    }
    if path.ends_with('/') && !normalized.ends_with('/') {
        normalized.push('/');
    }
    normalized
}

/// The number to name for the lesson at gallery path `target`: its `lesson.json` sequence, else the
/// number of a link text `Lesson <n>`.
fn lesson_number(root: &Path, target: &str, link_text: &str) -> Option<String> {
    let sequence = std::fs::read_to_string(root.join(target).join("lesson.json"))
        .ok()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .and_then(|lesson| lesson.get("sequence").and_then(Value::as_f64));
    match sequence {
        Some(sequence) if sequence.fract() == 0.0 && sequence.abs() < 1e21 => {
            Some(format!("{sequence:.0}"))
        }
        Some(sequence) => Some(sequence.to_string()),
        None => number_of_lesson_text(link_text),
    }
}

/// `n` of a link text `Lesson <n>`, in any letter case.
fn number_of_lesson_text(text: &str) -> Option<String> {
    let (word, rest) = text.split_at_checked(6)?;
    if !word.eq_ignore_ascii_case("lesson") {
        return None;
    }
    let digits = rest.trim_start();
    let blanks = rest.len() - digits.len();
    (blanks > 0 && !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.to_owned())
}

#[cfg(test)]
#[path = "readme_tests.rs"]
mod tests;
