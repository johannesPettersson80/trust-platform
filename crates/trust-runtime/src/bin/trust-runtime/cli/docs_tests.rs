#[cfg(test)]
mod docs_tests {
    //! The `trust-runtime` command lines that the documentation shows parse with this command line (checklist G16): every
    //! line of a fenced block in the README, the pages of `docs/public` and the examples' READMEs that starts with
    //! `trust-runtime` (after an optional `$ ` prompt) is split as a POSIX shell splits it, joined with its `\`
    //! continuations, cut at a comment, a pipe, a list operator or a redirection, and parsed with [`Cli`]. A renamed command
    //! or flag fails here instead of leaving the page behind. A placeholder such as `<project>` is a value; a line whose
    //! command is a placeholder, `trust-runtime <command>`, names no command and is left out.

    use std::fs;

    use clap::error::ErrorKind;
    use clap::Parser;
    use trust_test_support::doc_commands::{command_lines, pages, repository, words};

    use super::Cli;

    #[test]
    fn every_command_line_in_the_docs_parses() {
        let root = repository(env!("CARGO_MANIFEST_DIR"));
        let mut checked = 0;
        let mut failures = Vec::new();
        for page in pages(&root) {
            let Ok(text) = fs::read_to_string(&page) else {
                continue;
            };
            let shown = page.strip_prefix(&root).unwrap_or(&page).display().to_string();
            for (line, command) in command_lines(&text, "trust-runtime") {
                let Some(arguments) = words(&command) else {
                    failures.push(format!("{shown}:{line}: an unterminated quote: {command}"));
                    continue;
                };
                if arguments.get(1).is_none_or(|first| first.starts_with('<') || first.starts_with('[')) {
                    continue;
                }
                checked += 1;
                if let Err(error) = Cli::try_parse_from(&arguments) {
                    // A usage line, `trust-runtime bench <command>`, names its command by a placeholder.
                    let usage = error.kind() == ErrorKind::InvalidSubcommand
                        && arguments.iter().any(|word| word.starts_with('<'));
                    if !usage && !matches!(
                        error.kind(),
                        ErrorKind::DisplayHelp
                            | ErrorKind::DisplayVersion
                            | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand
                    ) {
                        let reason = error.to_string();
                        let reason = reason.lines().next().unwrap_or_default();
                        failures.push(format!("{shown}:{line}: {reason}: {command}"));
                    }
                }
            }
        }
        // The documentation shows about 260 such lines (14 November 2026, after the examples' tests moved to
        // `trust-dev test`, whose lines trust-dev's own test parses); far fewer means the pages were not found.
        assert!(checked >= 200, "only {checked} trust-runtime command lines were found");
        assert!(
            failures.is_empty(),
            "{} of {checked} trust-runtime command lines in the documentation do not parse:\n{}",
            failures.len(),
            failures.join("\n")
        );
    }
}
