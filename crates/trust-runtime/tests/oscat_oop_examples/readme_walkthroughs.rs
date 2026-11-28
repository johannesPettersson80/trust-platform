//! The OSCAT families' READMEs as they write them (specification 25 §3.5). Every
//! `trust-dev test --project examples/OSCAT/<family>/<variant>` that a converted family README
//! quotes runs, from the repository's root, or on a copy of the project after the README's
//! "`<file>`, change `<old>` to `<new>`" in the same list item, and its output holds the lines of the text
//! block that follows the command, in their order. The timings in brackets and parentheses differ
//! from run to run and are left out on both sides.

use std::collections::BTreeSet;

use super::*;

/// One `trust-dev test` run that a README quotes, with the edit that comes before it, if any.
struct QuotedRun {
    project: String,
    edit: Option<Edit>,
    expected: Vec<String>,
}

/// A README's "`<file>`, change `<old>` to `<new>`", the file relative to the family's folder.
struct Edit {
    file: String,
    old: String,
    new: String,
}

/// A fenced block of a README: its info string, its lines without their common indentation, and
/// the prose between the previous block and this one.
struct Fence {
    info: String,
    lines: Vec<String>,
    prose_before: String,
}

fn repository_root() -> PathBuf {
    examples_root().join("..")
}

fn fences(readme: &str) -> Vec<Fence> {
    let mut found = Vec::new();
    let mut prose = String::new();
    let mut lines = readme.lines();
    while let Some(line) = lines.next() {
        let Some(info) = line.trim_start().strip_prefix("```") else {
            prose.push_str(line);
            prose.push('\n');
            continue;
        };
        let indent = line.len() - line.trim_start().len();
        let mut body = Vec::new();
        for inner in lines.by_ref() {
            if inner.trim_start().starts_with("```") {
                break;
            }
            body.push(
                inner
                    .get(indent..)
                    .unwrap_or(inner.trim_start())
                    .to_string(),
            );
        }
        found.push(Fence {
            info: info.trim().to_string(),
            lines: body,
            prose_before: std::mem::take(&mut prose),
        });
    }
    found
}

/// The text between the first two backticks of `text` after `start`, and where it ends.
fn backticked(text: &str, start: usize) -> Option<(String, usize)> {
    let open = start + text[start..].find('`')?;
    let close = open + 1 + text[open + 1..].find('`')?;
    Some((text[open + 1..close].to_string(), close + 1))
}

/// The text of the last top-level list item of `prose`: a line that starts with `- ` or `<n>. `
/// and the lines after it, up to a heading.
fn last_item(prose: &str) -> String {
    let mut item = String::new();
    for line in prose.lines() {
        let numbered = line.split_once(". ").is_some_and(|(number, _)| {
            !number.is_empty() && number.chars().all(|c| c.is_ascii_digit())
        });
        if line.starts_with("- ") || line.starts_with('#') || numbered {
            item.clear();
        }
        item.push_str(line);
        item.push('\n');
    }
    item
}

/// The "`<file>`, change `<old>` to `<new>`" of the list item that holds a run, whatever its line
/// breaks: the file is the last backticked text before ", change".
fn edit_in(prose: &str) -> Option<Edit> {
    let text = last_item(prose)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let at = text.rfind(", change `")?;
    let file_end = text[..at].rfind('`')?;
    let file_start = text[..file_end].rfind('`')?;
    let file = text[file_start + 1..file_end].to_string();
    let (old, after_old) = backticked(&text, at + ", change ".len())?;
    let to = after_old + text[after_old..].find(" to `")? + " to ".len();
    let (new, _) = backticked(&text, to)?;
    Some(Edit { file, old, new })
}

fn quoted_runs(readme: &str) -> Vec<QuotedRun> {
    let blocks = fences(readme);
    let mut runs = Vec::new();
    for (index, fence) in blocks.iter().enumerate() {
        let command = fence.lines.join(" ");
        let Some(project) = command.trim().strip_prefix("trust-dev test --project ") else {
            continue;
        };
        if fence.info != "bash" || !project.starts_with("examples/OSCAT/") {
            continue;
        }
        let output = blocks
            .get(index + 1)
            .filter(|next| next.info == "text")
            .unwrap_or_else(|| panic!("the run of {project} is followed by its text block"));
        runs.push(QuotedRun {
            project: project.to_string(),
            edit: edit_in(&fence.prose_before),
            expected: output.lines.clone(),
        });
    }
    runs
}

/// `line` with each timing, `[12ms]` or `(4705ms)`, written as `[]` or `()`.
fn without_timings(line: &str) -> String {
    let mut out = String::new();
    let mut rest = line;
    while let Some(open) = rest.find(['[', '(']) {
        let close_char = if rest.as_bytes()[open] == b'[' {
            ']'
        } else {
            ')'
        };
        let Some(close) = rest[open..].find(close_char).map(|at| open + at) else {
            break;
        };
        let inner = &rest[open + 1..close];
        out.push_str(&rest[..open + 1]);
        if !(inner.ends_with("ms") && inner[..inner.len() - 2].chars().all(|c| c.is_ascii_digit()))
        {
            out.push_str(inner);
        }
        out.push(close_char);
        rest = &rest[close + 1..];
    }
    out.push_str(rest);
    out.trim_end().to_string()
}

/// The copy of `project` (relative to the repository) with `edit`, its library paths made absolute.
fn edited_copy(project: &str, edit: &Edit) -> TempProject {
    let source = repository_root().join(project);
    let family = source.parent().expect("the variant's family");
    let variant = source.file_name().expect("the variant").to_string_lossy();
    let file = edit
        .file
        .strip_prefix(&format!("{variant}/"))
        .unwrap_or_else(|| panic!("the edit's file {} lies in {variant}/", edit.file));
    let copy = TempProject::new(&format!("oscat-readme-{variant}"));
    for entry in walk(&source) {
        let relative = entry.strip_prefix(&source).expect("a file of the project");
        let target = copy.path().join(relative);
        std::fs::create_dir_all(target.parent().expect("a folder")).expect("the copy's folder");
        std::fs::copy(&entry, &target).expect("copy a project file");
    }
    let manifest = copy.path().join("trust-lsp.toml");
    let libraries = dependency_manifest_path(&repository_root().join("libraries"));
    let text = std::fs::read_to_string(&manifest).expect("the copy's trust-lsp.toml");
    std::fs::write(
        &manifest,
        text.replace(
            "path = \"../../../../libraries/",
            &format!("path = \"{libraries}/"),
        ),
    )
    .expect("rewrite the copy's library paths");
    let edited = copy.path().join(file);
    let text = std::fs::read_to_string(&edited)
        .unwrap_or_else(|error| panic!("read {} of {}: {error}", file, family.display()));
    assert_eq!(
        text.matches(&edit.old).count(),
        1,
        "`{}` occurs once in {}",
        edit.old,
        edit.file
    );
    std::fs::write(&edited, text.replacen(&edit.old, &edit.new, 1)).expect("write the edit");
    copy
}

fn walk(folder: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for entry in std::fs::read_dir(folder).expect("read a project folder") {
        let path = entry.expect("a folder entry").path();
        if path.is_dir() {
            files.extend(walk(&path));
        } else {
            files.push(path);
        }
    }
    files
}

/// The lines of `trust-dev test` on `project`, a path from the repository's root or a copy.
fn run_tests(project: &Path) -> Vec<String> {
    let output = Command::new(trust_dev_bin())
        .current_dir(repository_root())
        .args(["test", "--project"])
        .arg(project)
        .output()
        .expect("run trust-dev test");
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(without_timings)
        .collect()
}

/// The families whose README is converted: on no list of `scripts/example_readmes.json`.
pub(super) fn converted_families() -> Vec<String> {
    let lists = std::fs::read_to_string(repository_root().join("scripts/example_readmes.json"))
        .expect("scripts/example_readmes.json");
    oscat_example_dirs()
        .iter()
        .map(|dir| {
            dir.file_name()
                .expect("a family")
                .to_string_lossy()
                .into_owned()
        })
        .filter(|family| !lists.contains(&format!("\"examples/OSCAT/{family}/README.md\"")))
        .collect()
}

#[test]
fn every_converted_oscat_readme_quotes_the_runs_of_both_projects_as_they_go() {
    // Specification 25 §3.5: an example README walks through real runs, so each quoted line must
    // come out of the command it follows. Catches a README whose quoted test names, counts or
    // failure no longer match the projects, a quoted edit that no longer applies or no longer fails
    // as quoted, and a converted family whose walkthrough leaves out one of its two projects.
    let families = converted_families();
    for family in &families {
        let readme_path = oscat_examples_root().join(family).join("README.md");
        let readme = std::fs::read_to_string(&readme_path).expect("the family's README");
        let runs = quoted_runs(&readme);
        let variants: BTreeSet<&str> = runs
            .iter()
            .filter_map(|run| run.project.rsplit('/').next())
            .collect();
        assert!(
            variants.contains("non-oop") && variants.contains("oop"),
            "{} quotes runs of both projects, not only {variants:?}",
            readme_path.display()
        );
        for run in &runs {
            let copy = run
                .edit
                .as_ref()
                .map(|edit| edited_copy(&run.project, edit));
            let project = copy
                .as_ref()
                .map(|copy| copy.path().to_path_buf())
                .unwrap_or_else(|| PathBuf::from(&run.project));
            let output = run_tests(&project);
            let mut remaining = output.iter();
            for expected in &run.expected {
                let expected = without_timings(expected);
                assert!(
                    remaining.any(|line| *line == expected),
                    "{}: the run of {}{} has no line `{expected}` after the lines before it; it \
                     printed:\n{}",
                    readme_path.display(),
                    run.project,
                    run.edit
                        .as_ref()
                        .map(|edit| format!(" with `{}` changed to `{}`", edit.old, edit.new))
                        .unwrap_or_default(),
                    output.join("\n")
                );
            }
        }
    }
}

#[test]
fn quoted_runs_are_read_with_their_edits_and_timings_left_out() {
    // The parser this test program's walkthrough check relies on, on a README written for it.
    // Catches a run whose output block is lost, an edit sentence that a line break hides, and a
    // timing that would make every quoted line differ from the next run.
    let readme = "1. Run it:\n\n    ```bash\n    trust-dev test --project examples/OSCAT/x/oop\n    ```\n\n    \
                  ```text\n    PASS [1/1] TEST_PROGRAM::A (src/Tests.st) [3ms]\n    1 passed, 0 failed, \
                  0 errors (912ms)\n    ```\n\n- Keep it low: in `non-oop/src/Tests.st`, change\n  `In := REAL#12.0` to \
                  `In := REAL#9.0` and run it:\n\n    ```bash\n    trust-dev test --project \
                  examples/OSCAT/x/non-oop\n    ```\n\n    ```text\n    FAIL [1/1] x\n    ```\n";
    let runs = quoted_runs(readme);
    assert_eq!(runs.len(), 2);
    assert_eq!(runs[0].project, "examples/OSCAT/x/oop");
    assert!(runs[0].edit.is_none());
    assert_eq!(
        runs[0]
            .expected
            .iter()
            .map(|line| without_timings(line))
            .collect::<Vec<_>>(),
        [
            "PASS [1/1] TEST_PROGRAM::A (src/Tests.st) []",
            "1 passed, 0 failed, 0 errors ()"
        ]
    );
    let edit = runs[1].edit.as_ref().expect("the second run's edit");
    assert_eq!(
        (edit.file.as_str(), edit.old.as_str(), edit.new.as_str()),
        ("non-oop/src/Tests.st", "In := REAL#12.0", "In := REAL#9.0")
    );
}
