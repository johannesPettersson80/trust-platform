//! The command lines of a program that the documentation shows (checklist G16), for a command-line
//! tool's test to parse with its own parser, so that a renamed command or flag fails there instead of
//! leaving a page behind. The pages are the README, the pages of `docs/public` and the examples'
//! READMEs. A command line is a line of a fenced block that starts with the program's name, after
//! an optional `$ ` prompt, joined with its `\` continuations; it is split as a POSIX shell splits
//! it and cut at a comment, a pipe, a list operator or a redirection.

use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};

/// The repository's folder, from a crate's manifest folder `manifest_dir`, two folders up.
#[must_use]
pub fn repository(manifest_dir: &str) -> PathBuf {
    Path::new(manifest_dir).join("../..")
}

fn collect(folder: &Path, found: &mut Vec<PathBuf>, wanted: &dyn Fn(&Path) -> bool) {
    let Ok(entries) = fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if !matches!(
                entry.file_name().to_str(),
                Some("node_modules" | "target" | ".git")
            ) {
                collect(&path, found, wanted);
            }
        } else if wanted(&path) {
            found.push(path);
        }
    }
}

/// The documentation's pages under `root`: the README, the pages of `docs/public`, the deployment
/// notes of `docs/deploy` and the examples' READMEs, sorted.
#[must_use]
pub fn pages(root: &Path) -> Vec<PathBuf> {
    let mut found = vec![root.join("README.md")];
    collect(&root.join("docs/public"), &mut found, &|path| {
        path.extension().is_some_and(|extension| extension == "md")
    });
    // The deployment notes, which the runtime archives carry (26 November 2026: two of them, and the systemd
    // units, ran `trust-runtime --project …`, which the command line refuses).
    collect(&root.join("docs/deploy"), &mut found, &|path| {
        path.extension().is_some_and(|extension| extension == "md")
    });
    collect(&root.join("examples"), &mut found, &|path| {
        path.extension().is_some_and(|extension| extension == "md")
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("README"))
    });
    found.sort();
    found
}

/// The command lines of `program` in the fenced blocks of `page`, each with its first line in the
/// page.
#[must_use]
pub fn command_lines(page: &str, program: &str) -> Vec<(usize, String)> {
    let start_of = format!("{program} ");
    let mut found = Vec::new();
    let mut in_block = false;
    let mut pending: Option<(usize, String)> = None;
    for (index, raw) in page.lines().enumerate() {
        let line = raw.trim();
        if line.starts_with("```") {
            in_block = !in_block;
            pending = None;
            continue;
        }
        if !in_block {
            continue;
        }
        let (start, text) = if let Some((start, before)) = pending.take() {
            (start, format!("{before} {line}"))
        } else {
            let command = line.strip_prefix("$ ").unwrap_or(line);
            if !command.starts_with(&start_of) {
                continue;
            }
            (index + 1, command.to_owned())
        };
        if let Some(joined) = text.strip_suffix('\\') {
            pending = Some((start, joined.trim_end().to_owned()));
            continue;
        }
        found.push((start, text));
    }
    found
}

/// The words of `line` as a POSIX shell splits them, without expansion, up to a comment, a pipe, a
/// list operator or a redirection; `None` for an unterminated quote.
#[must_use]
pub fn words(line: &str) -> Option<Vec<String>> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut started = false;
    let mut characters = line.chars().peekable();
    while let Some(character) = characters.next() {
        match character {
            '\'' => {
                started = true;
                loop {
                    match characters.next()? {
                        '\'' => break,
                        other => word.push(other),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match characters.next()? {
                        '"' => break,
                        '\\' => word.push(characters.next()?),
                        other => word.push(other),
                    }
                }
            }
            '\\' => {
                started = true;
                word.push(characters.next().unwrap_or('\\'));
            }
            character if character.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            '#' if !started => break,
            // A placeholder such as `<project>` is one word, not a redirection.
            '>' if word.starts_with('<') => word.push('>'),
            '|' | ';' | '&' | '>' => {
                // `2>` redirects the error output: the 2 is no word.
                if character == '>' && word == "2" {
                    word.clear();
                    started = false;
                }
                break;
            }
            other => {
                started = true;
                word.push(other);
            }
        }
    }
    if started {
        words.push(word);
    }
    Some(words)
}

/// The commands that the table under the heading `heading` of the Markdown `page` names in its
/// first column (checklist G282c): the first word of each command in backquotes, so that
/// `` `hmi check` `` names `hmi` and `` `run` / `play` `` names `run` and `play`. The table ends at
/// the next heading; a page without the heading names none.
#[must_use]
pub fn table_commands(page: &str, heading: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut inside = false;
    for line in page.lines() {
        if line.starts_with('#') {
            inside = line.trim_end() == heading;
            continue;
        }
        let Some(row) = line.strip_prefix('|').filter(|_| inside) else {
            continue;
        };
        let first = row.split('|').next().unwrap_or_default();
        for quoted in first.split('`').skip(1).step_by(2) {
            if let Some(command) = quoted.split_whitespace().next() {
                found.insert(command.to_owned());
            }
        }
    }
    found
}

/// The id that the site gives a heading (Python-Markdown's `toc`, as `mkdocs.yml` configures it): its
/// text without the characters that are neither word characters, spaces nor `-`, trimmed, in small
/// letters, with each run of spaces and `-` one `-`. `Run and Play` is `run-and-play`.
#[must_use]
pub fn heading_anchor(heading: &str) -> String {
    let kept: String = heading
        .chars()
        .filter(|character| {
            character.is_alphanumeric()
                || *character == '_'
                || character.is_whitespace()
                || *character == '-'
        })
        .collect();
    let mut anchor = String::new();
    let mut gap = false;
    for character in kept.trim().to_lowercase().chars() {
        if character.is_whitespace() || character == '-' {
            gap = true;
            continue;
        }
        if gap && !anchor.is_empty() {
            anchor.push('-');
        }
        gap = false;
        anchor.push(character);
    }
    anchor
}

/// The ids of the headings of the Markdown `page` (`##` to `####`), as the site gives them.
#[must_use]
pub fn heading_anchors(page: &str) -> BTreeSet<String> {
    page.lines()
        .filter_map(|line| {
            let level = line
                .chars()
                .take_while(|character| *character == '#')
                .count();
            (2..=4)
                .contains(&level)
                .then(|| line[level..].trim())
                .filter(|text| !text.is_empty())
                .map(heading_anchor)
        })
        .collect()
}

/// The rows of the table under the heading `heading` of the Markdown `page`, each its first column
/// and the ids that the column's links name (`[`run`](#run-and-play)` names `run-and-play`); a row of
/// a command in backquotes counts, the header and the separator do not.
#[must_use]
pub fn table_links(page: &str, heading: &str) -> Vec<(String, Vec<String>)> {
    let mut rows = Vec::new();
    let mut inside = false;
    for line in page.lines() {
        if line.starts_with('#') {
            inside = line.trim_end() == heading;
            continue;
        }
        let Some(row) = line.strip_prefix('|').filter(|_| inside) else {
            continue;
        };
        let first = row.split('|').next().unwrap_or_default().trim();
        if !first.contains('`') {
            continue;
        }
        let links = first
            .split("](#")
            .skip(1)
            .filter_map(|rest| rest.split(')').next())
            .map(str::to_owned)
            .collect();
        rows.push((first.to_owned(), links));
    }
    rows
}

#[cfg(test)]
mod tests {
    use super::{
        command_lines, heading_anchor, heading_anchors, table_commands, table_links, words,
    };

    #[test]
    fn a_heading_takes_the_id_that_the_site_gives_it() {
        // Python-Markdown's slug: punctuation dropped, small letters, a run of spaces and `-` one
        // `-`. Catches an id with the `/` kept, with spaces, or with capital letters, and a page's
        // headings read without their level or beyond `####`.
        assert_eq!(heading_anchor("Run and Play"), "run-and-play");
        assert_eq!(
            heading_anchor("Build / validate / test"),
            "build-validate-test"
        );
        assert_eq!(
            heading_anchor("Entry-point and Error Contract"),
            "entry-point-and-error-contract"
        );
        assert_eq!(heading_anchor("HMI check"), "hmi-check");
        let page = "# Tool\n## Top\n### Run and Play\n##### Deep\nText\n";
        let ids: Vec<String> = heading_anchors(page).into_iter().collect();
        assert_eq!(ids, ["run-and-play", "top"]);
    }

    #[test]
    fn a_tables_row_gives_the_ids_its_links_name() {
        // A row of two linked commands gives both ids, a row without a link none. Catches a link of
        // the purpose column taken for the command's, and a header row counted.
        let page = "## Commands\n\n| Command | Purpose |\n| --- | --- |\n\
                    | [`run`](#run-and-play) / [`play`](#run-and-play) | see [it](#other) |\n\
                    | `ui` | a terminal UI |\n";
        assert_eq!(
            table_links(page, "## Commands"),
            [
                (
                    "[`run`](#run-and-play) / [`play`](#run-and-play)".to_owned(),
                    vec!["run-and-play".to_owned(), "run-and-play".to_owned()]
                ),
                ("`ui`".to_owned(), Vec::new()),
            ]
        );
    }

    #[test]
    fn a_tables_first_column_names_the_first_word_of_each_quoted_command() {
        // `hmi check` names `hmi`, `run` / `play` both, the header and the purpose column none, and
        // the table ends at the next heading. Catches a purpose's quoted word taken for a command,
        // a second word taken for one, and a row of the next table counted.
        let page = "# Tool\n\n## Commands\n\n| Command | Purpose |\n| --- | --- |\n\
                    | `run` / `play` | start `it` |\n| `hmi check` | check |\n\n## Next\n\n\
                    | `other` | x |\n";
        let names: Vec<String> = table_commands(page, "## Commands").into_iter().collect();
        assert_eq!(names, ["hmi", "play", "run"]);
        assert!(table_commands(page, "## Missing").is_empty());
    }

    #[test]
    fn a_command_line_is_split_as_a_shell_splits_it() {
        let split = |line: &str| words(line).expect("terminated");
        assert_eq!(
            split("trust-runtime build --project 'my plant' # build it"),
            ["trust-runtime", "build", "--project", "my plant"]
        );
        assert_eq!(
            split("trust-runtime ctl --project . status | jq .ok"),
            ["trust-runtime", "ctl", "--project", ".", "status"]
        );
        assert_eq!(
            split("trust-runtime run --project \"$PROJECT\" 2>&1 > log.txt"),
            ["trust-runtime", "run", "--project", "$PROJECT"]
        );
        assert_eq!(
            split("trust-runtime ctl --endpoint tcp://plc:9901 --token <token> status"),
            [
                "trust-runtime",
                "ctl",
                "--endpoint",
                "tcp://plc:9901",
                "--token",
                "<token>",
                "status"
            ]
        );
        assert_eq!(words("trust-runtime run --project 'unterminated"), None);
    }

    #[test]
    fn only_the_programs_lines_in_fenced_blocks_are_taken_with_their_continuations() {
        let page = "```bash\n$ trust-runtime build \\\n  --project .\ntrust-dev test --project .\necho done\n```\ntrust-runtime outside\n";
        assert_eq!(
            command_lines(page, "trust-runtime"),
            [(2, "trust-runtime build --project .".to_owned())]
        );
        assert_eq!(
            command_lines(page, "trust-dev"),
            [(4, "trust-dev test --project .".to_owned())]
        );
    }
}
