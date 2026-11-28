//! Helpers for the tests that run a tutorial's README on a copy of its project: the README, the
//! copy, `trust-dev test` in it, what a terminal shows of an answer, and the answers the README
//! quotes.

use std::path::{Path, PathBuf};
use std::process::Output;

use super::*;

/// The README of the tutorial in `examples/tutorials/<folder>`.
pub(super) fn readme(folder: &str) -> String {
    std::fs::read_to_string(tutorial_project_path(folder).join("README.md"))
        .unwrap_or_else(|error| panic!("{folder}'s README: {error}"))
}

/// A copy of the tutorial's project in `examples/tutorials/<folder>`, as its first step makes it.
pub(super) fn copied_project(folder: &str) -> PathBuf {
    let project = unique_temp_dir(folder);
    copy_folder(&tutorial_project_path(folder), &project);
    project
}

fn copy_folder(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("create the copy");
    for entry in std::fs::read_dir(from).expect("read the project") {
        let entry = entry.expect("a project entry");
        let target = to.join(entry.file_name());
        if entry.file_type().expect("its type").is_dir() {
            copy_folder(&entry.path(), &target);
        } else {
            std::fs::copy(entry.path(), &target).expect("copy a file");
        }
    }
}

/// `trust-dev test --project .` with `args`, run in `project` as the README has the reader run it.
pub(super) fn test_in(project: &Path, args: &[&str]) -> Output {
    trust_dev_command()
        .args(["test", "--project", "."])
        .args(args)
        .current_dir(project)
        .output()
        .expect("run trust-dev test")
}

/// What a terminal shows of `output`: its standard output, then its standard error, with the
/// paths written with `/`, as the README's Linux and macOS write them (Windows writes `\`).
pub(super) fn shown(output: &Output) -> String {
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    if cfg!(windows) {
        text.replace('\\', "/")
    } else {
        text
    }
}

/// `text` with each time that varies from run to run, such as `[0ms]` and `(265ms)`, written as
/// `[…ms]` and `(…ms)`.
pub(super) fn without_times(text: &str) -> String {
    let mut kept = String::with_capacity(text.len());
    let mut rest = text;
    while let Some(at) = rest.find(['[', '(']) {
        let (before, from) = rest.split_at(at);
        kept.push_str(before);
        let (open, after) = from.split_at(1);
        let close = if open == "[" { "ms]" } else { "ms)" };
        let digits = after.chars().take_while(char::is_ascii_digit).count();
        kept.push_str(open);
        if digits > 0 && after[digits..].starts_with(close) {
            kept.push('…');
            kept.push_str(close);
            rest = &after[digits + close.len()..];
        } else {
            rest = after;
        }
    }
    kept.push_str(rest);
    kept
}

/// The answers the README quotes, its `text` blocks, in their order, without the indentation of
/// the step that holds them.
pub(super) fn quoted_answers(readme: &str) -> Vec<String> {
    let mut answers = Vec::new();
    let mut open: Option<(usize, Vec<String>)> = None;
    for line in readme.lines() {
        let trimmed = line.trim_start();
        let indent = line.len() - trimmed.len();
        if let Some((fence, lines)) = open.as_mut() {
            if trimmed == "```" && indent == *fence {
                answers.push(lines.join("\n"));
                open = None;
            } else {
                lines.push(line.get(*fence..).unwrap_or_default().to_string());
            }
        } else if trimmed == "```text" {
            open = Some((indent, Vec::new()));
        }
    }
    answers
}

/// Replaces `from` with `to` once in the project's `file`, as the README has the reader edit it.
pub(super) fn edit(project: &Path, file: &str, from: &str, to: &str) {
    let path = project.join(file);
    let text = std::fs::read_to_string(&path).expect("read the file");
    assert_eq!(text.matches(from).count(), 1, "{file} holds `{from}` once");
    std::fs::write(&path, text.replacen(from, to, 1)).expect("write the file");
}
