#[cfg(test)]
mod docs_tests {
    //! The `trust-runtime` command lines that the documentation shows parse with this command line (checklist G16): every
    //! line of a fenced block in the README, the pages of `docs/public` and the examples' READMEs that starts with
    //! `trust-runtime` (after an optional `$ ` prompt) is split as a POSIX shell splits it, joined with its `\`
    //! continuations, cut at a comment, a pipe, a list operator or a redirection, and parsed with [`Cli`]. A renamed command
    //! or flag fails here instead of leaving the page behind. A placeholder such as `<project>` is a value; a line whose
    //! command is a placeholder, `trust-runtime <command>`, names no command and is left out.

    use std::fs;
    use std::path::{Path, PathBuf};

    use clap::error::ErrorKind;
    use clap::Parser;

    use super::Cli;

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    fn collect(folder: &Path, found: &mut Vec<PathBuf>, wanted: &dyn Fn(&Path) -> bool) {
        let Ok(entries) = fs::read_dir(folder) else {
            return;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if !matches!(entry.file_name().to_str(), Some("node_modules" | "target" | ".git")) {
                    collect(&path, found, wanted);
                }
            } else if wanted(&path) {
                found.push(path);
            }
        }
    }

    /// The README, every page of `docs/public` and every README of `examples`.
    fn pages(root: &Path) -> Vec<PathBuf> {
        let mut found = vec![root.join("README.md")];
        collect(&root.join("docs/public"), &mut found, &|path| {
            path.extension().is_some_and(|extension| extension == "md")
        });
        collect(&root.join("examples"), &mut found, &|path| {
            path.file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.starts_with("README") && name.ends_with(".md"))
        });
        found.sort();
        found
    }

    /// The `trust-runtime` command lines of the fenced blocks of `page`, each with its first line in the page.
    fn command_lines(page: &str) -> Vec<(usize, String)> {
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
            let (start, mut text) = match pending.take() {
                Some((start, before)) => (start, format!("{before} {line}")),
                None => {
                    let command = line.strip_prefix("$ ").unwrap_or(line);
                    if !command.starts_with("trust-runtime ") {
                        continue;
                    }
                    (index + 1, command.to_owned())
                }
            };
            if let Some(joined) = text.strip_suffix('\\') {
                text = joined.trim_end().to_owned();
                pending = Some((start, text));
                continue;
            }
            found.push((start, text));
        }
        found
    }

    /// The words of `line` as a POSIX shell splits them, without expansion, up to a comment, a pipe, a list operator or a
    /// redirection; `None` for an unterminated quote.
    fn words(line: &str) -> Option<Vec<String>> {
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

    #[test]
    fn every_command_line_in_the_docs_parses() {
        let root = root();
        let mut checked = 0;
        let mut failures = Vec::new();
        for page in pages(&root) {
            let Ok(text) = fs::read_to_string(&page) else {
                continue;
            };
            let shown = page.strip_prefix(&root).unwrap_or(&page).display().to_string();
            for (line, command) in command_lines(&text) {
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
        // The documentation shows about 470 such lines (30 October 2026); far fewer means the pages were not found.
        assert!(checked >= 300, "only {checked} trust-runtime command lines were found");
        assert!(
            failures.is_empty(),
            "{} of {checked} trust-runtime command lines in the documentation do not parse:\n{}",
            failures.len(),
            failures.join("\n")
        );
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
            ["trust-runtime", "ctl", "--endpoint", "tcp://plc:9901", "--token", "<token>", "status"]
        );
        assert_eq!(words("trust-runtime run --project 'unterminated"), None);
        assert_eq!(
            command_lines("```bash\n$ trust-runtime build \\\n  --project .\necho done\n```\ntrust-runtime outside\n"),
            [(2, "trust-runtime build --project .".to_owned())]
        );
    }
}
