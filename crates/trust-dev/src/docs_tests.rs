//! The `trust-dev` command lines that the documentation shows parse with this command line
//! (checklist G16), as `trust-runtime`'s do with its own: every line of a fenced block in the README,
//! the pages of `docs/public` and the examples' READMEs that starts with `trust-dev`, after an
//! optional `$ ` prompt, is split as a POSIX shell splits it and parsed with [`Cli`]. A renamed
//! command or flag fails here instead of leaving the page behind. A line whose command is a
//! placeholder, `trust-dev <command>`, names no command and is left out.

use std::collections::BTreeSet;
use std::fs;

use clap::error::ErrorKind;
use clap::{CommandFactory, Parser};
use trust_test_support::doc_commands::{
    command_lines, heading_anchors, pages, repository, table_commands, table_links, words,
};

use super::Cli;

#[test]
fn the_reference_names_every_command_that_the_help_shows() {
    // The CLI reference's table "Command Families" names every command that `trust-dev --help`
    // shows, and no other (checklist G282c). Catches a new command left out of the reference, and
    // a removed or renamed one that stays in it.
    let root = repository(env!("CARGO_MANIFEST_DIR"));
    let page = fs::read_to_string(root.join("docs/public/reference/cli/trust-dev.md"))
        .expect("the reference");
    let listed = table_commands(&page, "## Command Families");
    let shown: BTreeSet<String> = Cli::command()
        .get_subcommands()
        .filter(|command| !command.is_hide_set())
        .map(|command| command.get_name().to_owned())
        .collect();
    let missing: Vec<&String> = shown.difference(&listed).collect();
    let unknown: Vec<&String> = listed.difference(&shown).collect();
    assert!(
        missing.is_empty() && unknown.is_empty(),
        "the table lacks {missing:?}, and names {unknown:?}, which --help does not show"
    );
}

#[test]
fn every_command_of_the_reference_links_to_its_section() {
    // Each row of the table "Command Families" links its commands to sections of the page, by the
    // ids that the site gives their headings (checklist G282c). Catches a row without a link, and a
    // link whose section was renamed or removed.
    let root = repository(env!("CARGO_MANIFEST_DIR"));
    let page = fs::read_to_string(root.join("docs/public/reference/cli/trust-dev.md"))
        .expect("the reference");
    let anchors = heading_anchors(&page);
    let mut problems = Vec::new();
    for (row, links) in table_links(&page, "## Command Families") {
        if links.is_empty() {
            problems.push(format!("{row}: no link to its section"));
        }
        for link in links.iter().filter(|link| !anchors.contains(*link)) {
            problems.push(format!("{row}: #{link} names no heading of the page"));
        }
    }
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn every_command_line_in_the_docs_parses() {
    let root = repository(env!("CARGO_MANIFEST_DIR"));
    let mut checked = 0;
    let mut failures = Vec::new();
    for page in pages(&root) {
        let Ok(text) = fs::read_to_string(&page) else {
            continue;
        };
        let shown = page
            .strip_prefix(&root)
            .unwrap_or(&page)
            .display()
            .to_string();
        for (line, command) in command_lines(&text, "trust-dev") {
            let Some(arguments) = words(&command) else {
                failures.push(format!("{shown}:{line}: an unterminated quote: {command}"));
                continue;
            };
            if arguments
                .get(1)
                .is_none_or(|first| first.starts_with('<') || first.starts_with('['))
            {
                continue;
            }
            checked += 1;
            if let Err(error) = Cli::try_parse_from(&arguments) {
                // A usage line names its command by a placeholder.
                let usage = error.kind() == ErrorKind::InvalidSubcommand
                    && arguments.iter().any(|word| word.starts_with('<'));
                if !usage
                    && !matches!(
                        error.kind(),
                        ErrorKind::DisplayHelp
                            | ErrorKind::DisplayVersion
                            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    )
                {
                    let reason = error.to_string();
                    let reason = reason.lines().next().unwrap_or_default();
                    failures.push(format!("{shown}:{line}: {reason}: {command}"));
                }
            }
        }
    }
    // The documentation shows about 217 such lines (14 November 2026, most of them the examples'
    // `trust-dev test`); far fewer means the pages were not found.
    assert!(
        checked >= 150,
        "only {checked} trust-dev command lines were found"
    );
    assert!(
        failures.is_empty(),
        "{} of {checked} trust-dev command lines in the documentation do not parse:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
