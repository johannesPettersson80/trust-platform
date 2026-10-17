//! The `.gitignore` line that keeps a project's endpoint folder out of version control (spec 11,
//! section 6.8.0a). Every command that creates a project calls [`ensure_endpoint_dir_ignored`].

use std::fs;
use std::io;
use std::path::Path;

/// The line that a project's `.gitignore` carries for the endpoint folder.
pub const ENDPOINT_DIR_GITIGNORE_LINE: &str = "/.trust-runtime/";

/// The lines for the endpoint folder: a new `.gitignore` of a project creator without its own
/// template, and the lines appended to an existing `.gitignore` that does not list the folder yet.
pub const ENDPOINT_DIR_GITIGNORE: &str =
    "# The runtime's endpoint file: it can hold a control token.\n/.trust-runtime/\n";

/// Makes `<project>/.gitignore` list the endpoint folder. A missing file is written with
/// `default_text`, which lists the folder. An existing file is kept as it is, and the folder's line
/// is appended when no line ignores the folder yet.
///
/// # Errors
///
/// Returns an error when the file cannot be read or written.
pub fn ensure_endpoint_dir_ignored(project_root: &Path, default_text: &str) -> io::Result<()> {
    let path = project_root.join(".gitignore");
    match fs::read_to_string(&path) {
        Ok(text) if lists_endpoint_dir(&text) => Ok(()),
        Ok(mut text) => {
            if !text.is_empty() && !text.ends_with('\n') {
                text.push('\n');
            }
            text.push_str(ENDPOINT_DIR_GITIGNORE);
            fs::write(&path, text)
        }
        Err(error) if error.kind() == io::ErrorKind::NotFound => fs::write(&path, default_text),
        Err(error) => Err(error),
    }
}

/// Whether a `.gitignore` already has a line that ignores the endpoint folder at the project root.
fn lists_endpoint_dir(text: &str) -> bool {
    text.lines().map(str::trim).any(|line| {
        matches!(
            line,
            "/.trust-runtime/"
                | "/.trust-runtime"
                | ".trust-runtime/"
                | ".trust-runtime"
                | "**/.trust-runtime/"
                | "**/.trust-runtime"
        )
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    const DEFAULT: &str = "# default\n/.trust-runtime/\n";

    /// A scratch project folder, removed when the test ends.
    struct Scratch(PathBuf);

    impl Scratch {
        fn new(label: &str) -> Self {
            let nanos = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_nanos())
                .unwrap_or_default();
            let path = std::env::temp_dir().join(format!(
                "trust-endpoint-gitignore-{label}-{}-{nanos}",
                std::process::id()
            ));
            fs::create_dir_all(&path).expect("create scratch project");
            Self(path)
        }

        fn gitignore(&self) -> String {
            fs::read_to_string(self.0.join(".gitignore")).expect("read .gitignore")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn a_missing_gitignore_is_written_with_the_default_text() {
        let project = Scratch::new("missing");

        ensure_endpoint_dir_ignored(&project.0, DEFAULT).expect("ensure");

        assert_eq!(project.gitignore(), DEFAULT);
    }

    #[test]
    fn an_existing_gitignore_keeps_its_rules_and_gains_the_endpoint_folder() {
        let project = Scratch::new("append");
        fs::write(project.0.join(".gitignore"), "target/\n*.log").expect("write .gitignore");

        ensure_endpoint_dir_ignored(&project.0, DEFAULT).expect("ensure");

        assert_eq!(
            project.gitignore(),
            format!("target/\n*.log\n{ENDPOINT_DIR_GITIGNORE}")
        );
    }

    #[test]
    fn a_gitignore_that_lists_the_endpoint_folder_is_left_as_it_is() {
        for listed in [
            "/.trust-runtime/",
            ".trust-runtime",
            "  **/.trust-runtime/  ",
        ] {
            let project = Scratch::new("listed");
            let text = format!("target/\n{listed}\n");
            fs::write(project.0.join(".gitignore"), &text).expect("write .gitignore");

            ensure_endpoint_dir_ignored(&project.0, DEFAULT).expect("ensure");

            assert_eq!(project.gitignore(), text, "{listed:?}");
        }
    }

    #[test]
    fn the_line_constant_is_the_one_the_appended_lines_carry() {
        assert!(ENDPOINT_DIR_GITIGNORE
            .lines()
            .any(|line| line == ENDPOINT_DIR_GITIGNORE_LINE));
        assert!(lists_endpoint_dir(ENDPOINT_DIR_GITIGNORE));
    }
}
