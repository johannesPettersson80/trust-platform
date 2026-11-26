#[cfg(test)]
mod docs_tests {
    //! The `trust-runtime` command lines that the documentation shows parse with this command line (checklist G16): every
    //! line of a fenced block in the README, the pages of `docs/public`, the deployment notes of `docs/deploy` and the
    //! examples' READMEs that starts with
    //! `trust-runtime` (after an optional `$ ` prompt) is split as a POSIX shell splits it, joined with its `\`
    //! continuations, cut at a comment, a pipe, a list operator or a redirection, and parsed with [`Cli`]. A renamed command
    //! or flag fails here instead of leaving the page behind. A placeholder such as `<project>` is a value; a line whose
    //! command is a placeholder, `trust-runtime <command>`, names no command and is left out.
    //!
    //! The page "Create A New Project" lists the files of the project that `trust-runtime new` writes.

    use std::collections::BTreeSet;
    use std::fs;
    use std::path::Path;

    use clap::error::ErrorKind;
    use clap::{CommandFactory, Parser};
    use trust_runtime::new_project::create_project;
    use trust_test_support::doc_commands::{
        command_lines, heading_anchors, pages, repository, table_commands, table_links, words,
    };
    use trust_test_support::unique_temp_path;

    use super::{Cli, Command};

    #[test]
    fn the_reference_names_every_command_that_the_help_shows() {
        // The CLI reference's table "Top-level Command Families" names every top-level command that
        // `trust-runtime --help` shows, and no other (checklist G282c). Catches a new command left
        // out of the reference, and a removed or renamed one that stays in it.
        let root = repository(env!("CARGO_MANIFEST_DIR"));
        let page = fs::read_to_string(root.join("docs/public/reference/cli/trust-runtime.md"))
            .expect("the reference");
        let listed = table_commands(&page, "## Top-level Command Families");
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
        // Each row of the table "Top-level Command Families" links its commands to sections of the
        // page, by the ids that the site gives their headings (checklist G282c). Catches a row
        // without a link, and a link whose section was renamed or removed.
        let root = repository(env!("CARGO_MANIFEST_DIR"));
        let page = fs::read_to_string(root.join("docs/public/reference/cli/trust-runtime.md"))
            .expect("the reference");
        let anchors = heading_anchors(&page);
        let mut problems = Vec::new();
        for (row, links) in table_links(&page, "## Top-level Command Families") {
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
    fn the_service_units_start_the_runtime_warm() {
        // Specification 11, "Service units": the systemd units of `docs/deploy/systemd`, which the runtime archives
        // carry, run `trust-runtime run` on the deployed project with `--restart warm`, so that retained values come
        // back after a power loss, a crash or a restart by systemd. Catches a unit whose command line the CLI refuses
        // (both ran `trust-runtime --project ...`, refused as an unexpected argument, so systemd restarted the service
        // every 2 s and the PLC never ran), and a unit that starts cold and so drops the retained values at every boot.
        let folder = repository(env!("CARGO_MANIFEST_DIR")).join("docs/deploy/systemd");
        let mut units = 0;
        for entry in fs::read_dir(&folder).expect("read the units' folder") {
            let path = entry.expect("a folder entry").path();
            if path.extension().is_none_or(|extension| extension != "service") {
                continue;
            }
            units += 1;
            let text = fs::read_to_string(&path).expect("read a unit");
            let starts: Vec<&str> = text.lines().filter_map(|line| line.strip_prefix("ExecStart=")).collect();
            assert_eq!(starts.len(), 1, "{}: one ExecStart", path.display());
            let mut arguments = words(starts[0]).expect("a command line");
            assert_eq!(arguments[0], "/usr/local/bin/trust-runtime", "{}", path.display());
            arguments[0] = "trust-runtime".to_string();
            let cli = Cli::try_parse_from(&arguments).unwrap_or_else(|error| panic!("{}: {error}", path.display()));
            match cli.command {
                Some(Command::Run { project, restart, .. }) => {
                    assert_eq!(project.as_deref(), Some(Path::new("/opt/trust/current")), "{}", path.display());
                    assert_eq!(restart, "warm", "{}", path.display());
                }
                other => panic!("{}: ExecStart runs `trust-runtime run`, not {other:?}", path.display()),
            }
        }
        assert_eq!(units, 2, "the two units of docs/deploy/systemd");
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

    /// The files of the tree that "Create A New Project" shows under `my-first-plc/`: two spaces per level, a folder
    /// ends in `/`, and `#` starts a comment.
    fn listed_files(page: &str) -> Vec<String> {
        let tree = page
            .split("```text\nmy-first-plc/\n")
            .nth(1)
            .and_then(|rest| rest.split("```").next())
            .expect("the page shows the tree of my-first-plc/");
        let mut folders: Vec<&str> = Vec::new();
        let mut files = Vec::new();
        for line in tree.lines() {
            let entry = line.split('#').next().unwrap_or_default().trim_end();
            let name = entry.trim_start();
            if name.is_empty() {
                continue;
            }
            let depth = ((entry.len() - name.len()) / 2)
                .checked_sub(1)
                .expect("each entry of the tree is indented under my-first-plc/");
            folders.truncate(depth);
            match name.strip_suffix('/') {
                Some(folder) => folders.push(folder),
                None => files.push([folders.as_slice(), &[name]].concat().join("/")),
            }
        }
        files.sort();
        files
    }

    /// The files under `folder`, relative to `root` and joined with `/` on every platform.
    fn written_files(root: &Path, folder: &Path, found: &mut Vec<String>) {
        for entry in fs::read_dir(folder).expect("read a folder of the project") {
            let path = entry.expect("a folder entry").path();
            if path.is_dir() {
                written_files(root, &path, found);
            } else {
                let relative = path.strip_prefix(root).expect("a file of the project");
                let parts: Vec<String> = relative
                    .components()
                    .map(|part| part.as_os_str().to_string_lossy().into_owned())
                    .collect();
                found.push(parts.join("/"));
            }
        }
    }

    #[test]
    fn create_a_new_project_lists_the_files_that_trust_runtime_new_writes() {
        // Catches the page and the scaffold drifting apart: until 14 November 2026 the page listed five files of a new
        // project, which holds eight; `.gitignore` and the two files of `.vscode` were missing.
        let parent = unique_temp_path("trust-runtime-docs-new-project");
        fs::create_dir_all(&parent).expect("create the parent folder");
        let created = create_project(&parent, "my-first-plc", false).map(|project| {
            let mut written = Vec::new();
            written_files(&project, &project, &mut written);
            written
        });
        let _ = fs::remove_dir_all(&parent);
        let mut written = created.expect("trust-runtime new creates the project");
        written.sort();
        let page = fs::read_to_string(
            repository(env!("CARGO_MANIFEST_DIR")).join("docs/public/start/create-new-project.md"),
        )
        .expect("read the page");
        assert_eq!(listed_files(&page), written);
    }
}
