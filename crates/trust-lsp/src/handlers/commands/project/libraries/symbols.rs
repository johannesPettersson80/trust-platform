//! The symbols a library offers in the Libraries panel (specification 25 §7): every
//! `FUNCTION_BLOCK`, `FUNCTION` and `TYPE` that starts a line of one of its `.st` or `.pou` files,
//! with the line that declares it and the snippet the panel inserts into a `VAR` block or copies.

use std::cmp::Ordering;
use std::collections::HashSet;
use std::path::Path;

use serde::Serialize;

/// One symbol of a library.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(super) struct SymbolSummary {
    pub(super) name: String,
    /// `function_block`, `function` or `type`.
    pub(super) kind: &'static str,
    /// The file that declares it.
    pub(super) file: String,
    /// The line that declares it, trimmed, with its blanks collapsed.
    pub(super) declaration: String,
    /// What the panel inserts: `<instance> : <FB>;`, `value : <TYPE>;` or `<FUNCTION>(...)`.
    pub(super) snippet: String,
}

/// The keywords that declare a symbol, longest first so `FUNCTION_BLOCK` is not read as
/// `FUNCTION`.
const KEYWORDS: [(&str, &str); 3] = [
    ("FUNCTION_BLOCK", "function_block"),
    ("FUNCTION", "function"),
    ("TYPE", "type"),
];

/// The `.st` and `.pou` files (any letter case) below the library folder `root`, in path order.
pub(super) fn library_sources(root: &Path) -> Vec<std::path::PathBuf> {
    let mut files = Vec::new();
    collect_sources(root, &mut files);
    files.sort();
    files
}

/// The symbols of the library files `files` (in their order): each kind and name once, the first
/// declaration kept; ordered by kind, then name.
pub(super) fn library_symbols(files: &[std::path::PathBuf]) -> Vec<SymbolSummary> {
    let sources: Vec<(String, String)> = files
        .iter()
        .filter_map(|file| {
            let bytes = std::fs::read(file).ok()?;
            Some((
                file.to_string_lossy().into_owned(),
                String::from_utf8_lossy(&bytes).into_owned(),
            ))
        })
        .collect();
    summarize(&sources)
}

fn collect_sources(folder: &Path, files: &mut Vec<std::path::PathBuf>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        let path = entry.path();
        if file_type.is_dir() {
            collect_sources(&path, files);
        } else if file_type.is_file() && is_source(&path) {
            files.push(path);
        }
    }
}

fn is_source(path: &Path) -> bool {
    path.extension()
        .and_then(|extension| extension.to_str())
        .is_some_and(|extension| {
            extension.eq_ignore_ascii_case("st") || extension.eq_ignore_ascii_case("pou")
        })
}

/// The symbols of `sources` (each a file and its text), deduplicated and ordered.
pub(super) fn summarize(sources: &[(String, String)]) -> Vec<SymbolSummary> {
    let mut seen = HashSet::new();
    let mut symbols = Vec::new();
    for (file, text) in sources {
        for (kind, name, declaration) in declarations(text) {
            if !seen.insert(format!("{kind}:{name}")) {
                continue;
            }
            symbols.push(SymbolSummary {
                snippet: snippet(kind, &name),
                name,
                kind,
                file: file.clone(),
                declaration,
            });
        }
    }
    symbols.sort_by(|left, right| {
        locale_order(left.kind, right.kind).then_with(|| locale_order(&left.name, &right.name))
    });
    symbols
}

/// Each `(kind, name, declaration line)` of `text`: a keyword at the start of a line (after blanks,
/// which may span empty lines), then at least one blank, then the name. The search goes on after
/// the name, at the next line start.
fn declarations(text: &str) -> Vec<(&'static str, String, String)> {
    let bytes = text.as_bytes();
    let mut found = Vec::new();
    let mut search = 0;
    while search < text.len() {
        let line_start = if search == 0 || bytes[search - 1] == b'\n' {
            search
        } else {
            match bytes[search..].iter().position(|byte| *byte == b'\n') {
                Some(newline) => search + newline + 1,
                None => break,
            }
        };
        match declaration_at(text, line_start) {
            Some(found_here) => {
                found.push((
                    found_here.kind,
                    text[found_here.name_start..found_here.name_end].to_owned(),
                    declaration_line(text, found_here.keyword_start),
                ));
                search = found_here.name_end;
            }
            None => search = line_start + 1,
        }
    }
    found
}

/// A declaration found at a line start.
struct Found {
    kind: &'static str,
    keyword_start: usize,
    name_start: usize,
    name_end: usize,
}

/// The declaration that starts at the line start `start`, if one does.
fn declaration_at(text: &str, start: usize) -> Option<Found> {
    let rest = &text[start..];
    let keyword_start = start + (rest.len() - rest.trim_start().len());
    let after_blanks = &text[keyword_start..];
    for (keyword, kind) in KEYWORDS {
        let Some(head) = after_blanks.get(..keyword.len()) else {
            continue;
        };
        if !head.eq_ignore_ascii_case(keyword) {
            continue;
        }
        let after_keyword = &after_blanks[keyword.len()..];
        let name = after_keyword.trim_start();
        if name.len() == after_keyword.len() {
            continue;
        }
        let mut chars = name.char_indices();
        match chars.next() {
            Some((_, first)) if first.is_ascii_alphabetic() || first == '_' => {}
            _ => continue,
        }
        let name_length = chars
            .find(|(_, ch)| !(ch.is_ascii_alphanumeric() || *ch == '_'))
            .map_or(name.len(), |(index, _)| index);
        let name_start = text.len() - name.len();
        return Some(Found {
            kind,
            keyword_start,
            name_start,
            name_end: name_start + name_length,
        });
    }
    None
}

/// The line that holds `offset`, trimmed, with every run of blanks made one space.
fn declaration_line(text: &str, offset: usize) -> String {
    let line_start = text[..offset].rfind('\n').map_or(0, |newline| newline + 1);
    let line_end = text[offset..]
        .find('\n')
        .map_or(text.len(), |newline| offset + newline);
    text[line_start..line_end]
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// The snippet the panel inserts for a symbol of `kind` named `name`.
pub(super) fn snippet(kind: &str, name: &str) -> String {
    match kind {
        "function_block" => format!("{} : {name};", instance_name(name)),
        "type" => format!("value : {name};"),
        _ => format!("{name}(...)"),
    }
}

/// The instance name of a function block: its name without characters other than letters, digits
/// and `_`, the first one lower case; `libraryBlock` when nothing is left.
fn instance_name(name: &str) -> String {
    let base: String = name
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect();
    let mut chars = base.chars();
    match chars.next() {
        Some(first) => first.to_lowercase().chain(chars).collect(),
        None => "libraryBlock".to_owned(),
    }
}

/// The order the panel lists names in, as a person reads them: letter case is compared last, with
/// the small letter first, and `_` sorts before digits and letters.
pub(super) fn locale_order(left: &str, right: &str) -> Ordering {
    fn primary(ch: char) -> (u8, char) {
        if ch == '_' {
            (0, ch)
        } else {
            (1, ch.to_ascii_lowercase())
        }
    }
    left.chars()
        .map(primary)
        .cmp(right.chars().map(primary))
        .then_with(|| {
            left.chars()
                .map(|ch| !ch.is_ascii_lowercase())
                .cmp(right.chars().map(|ch| !ch.is_ascii_lowercase()))
        })
}

#[cfg(test)]
#[path = "symbols_tests.rs"]
mod tests;
