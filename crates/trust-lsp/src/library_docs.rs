//! External library documentation ingestion (markdown headings).

use rustc_hash::FxHashMap;
use std::fs;

use crate::config::ProjectConfig;

pub(crate) fn library_doc_map(config: &ProjectConfig) -> FxHashMap<String, String> {
    let mut docs = FxHashMap::default();
    for lib in &config.libraries {
        for path in &lib.docs {
            if let Ok(contents) = fs::read_to_string(path) {
                parse_markdown_docs(&contents, &mut docs);
            }
        }
    }
    docs
}

fn parse_markdown_docs(contents: &str, docs: &mut FxHashMap<String, String>) {
    let mut current: Option<String> = None;
    let mut buffer: Vec<String> = Vec::new();
    let mut fence: Option<char> = None;

    let flush =
        |name: Option<String>, buffer: &mut Vec<String>, docs: &mut FxHashMap<String, String>| {
            let Some(name) = name else {
                buffer.clear();
                return;
            };
            let text = buffer.join("\n").trim().to_string();
            buffer.clear();
            if !text.is_empty() {
                docs.insert(name, text);
            }
        };

    for line in contents.lines() {
        if let Some(active_fence) = fence {
            buffer.push(line.to_string());
            if fence_marker(line) == Some(active_fence) {
                fence = None;
            }
            continue;
        }
        if let Some(marker) = fence_marker(line) {
            buffer.push(line.to_string());
            fence = Some(marker);
            continue;
        }
        if let Some(heading) = markdown_heading(line) {
            flush(current.take(), &mut buffer, docs);
            if !heading.is_empty() {
                current = Some(heading.to_ascii_uppercase());
            }
        } else {
            buffer.push(line.to_string());
        }
    }

    flush(current.take(), &mut buffer, docs);
}

fn fence_marker(line: &str) -> Option<char> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next()?;
    if !matches!(marker, '`' | '~') {
        return None;
    }
    (trimmed
        .chars()
        .take_while(|character| *character == marker)
        .count()
        >= 3)
        .then_some(marker)
}

fn markdown_heading(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    let marker_count = trimmed.bytes().take_while(|byte| *byte == b'#').count();
    if !(1..=6).contains(&marker_count) {
        return None;
    }
    let rest = &trimmed[marker_count..];
    if rest
        .chars()
        .next()
        .is_some_and(|character| !character.is_whitespace())
    {
        return None;
    }
    Some(rest.trim())
}

pub(crate) fn doc_for_name<'a>(docs: &'a FxHashMap<String, String>, name: &str) -> Option<&'a str> {
    docs.get(&name.to_ascii_uppercase())
        .map(|value| value.as_str())
}

#[cfg(test)]
#[path = "library_docs/contract_tests.rs"]
mod contract_tests;
