//! `trust-lsp check` as a command (docs/specs/14-lsp.md §7.7): the editor's diagnostics of a
//! project on stdout, a summary on stderr, and a usage error for a file the project does
//! not contain.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// A two-file project in a fresh temporary directory, removed on drop.
struct Project(PathBuf);

impl Project {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "trust-lsp-check-command-{name}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("src dir");
        std::fs::write(
            root.join("trust-lsp.toml"),
            "[project]\ninclude_paths = [\"src\"]\n",
        )
        .expect("config");
        std::fs::write(
            root.join("src").join("Types.st"),
            "TYPE Point : STRUCT x : INT; END_STRUCT END_TYPE\n",
        )
        .expect("types");
        std::fs::write(
            root.join("src").join("Main.st"),
            "PROGRAM Main\nVAR p : Point; END_VAR\np.x := missing;\nEND_PROGRAM\n",
        )
        .expect("main");
        Self(root)
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn check(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(
        std::env::var_os("CARGO_BIN_EXE_trust-lsp")
            .expect("Cargo must provide trust-lsp binary while executing integration tests"),
    )
    .arg("check")
    .args(args)
    .env_remove("RUST_LOG")
    .output()
    .expect("run trust-lsp check")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8(bytes.to_vec()).expect("utf-8 output")
}

#[test]
fn check_prints_each_diagnostic_with_its_native_relative_path() {
    let project = Project::new("text");
    let output = check(&[std::ffi::OsStr::new("--project"), project.0.as_os_str()]);
    let main = Path::new("src").join("Main.st").display().to_string();
    assert_eq!(
        output.status.code(),
        Some(1),
        "stderr: {}",
        text(&output.stderr)
    );
    assert_eq!(
        text(&output.stdout),
        format!(
            "{main}:1:9: warning[W009]: unused program 'Main'\n\
             {main}:3:7: error[E101]: undefined identifier 'missing'\n"
        )
    );
    assert_eq!(
        text(&output.stderr),
        "trust-lsp check: 1 error(s), 1 warning(s) in 2 file(s)\n"
    );
}

#[test]
fn check_rejects_a_file_outside_the_project() {
    let project = Project::new("outside");
    let outside = std::env::temp_dir().join(format!(
        "trust-lsp-check-command-outside-{}.st",
        std::process::id()
    ));
    std::fs::write(&outside, "PROGRAM Elsewhere\nEND_PROGRAM\n").expect("outside file");
    let output = check(&[
        std::ffi::OsStr::new("--project"),
        project.0.as_os_str(),
        std::ffi::OsStr::new("--file"),
        outside.as_os_str(),
    ]);
    let _ = std::fs::remove_file(&outside);
    assert_eq!(
        output.status.code(),
        Some(2),
        "stdout: {}\nstderr: {}",
        text(&output.stdout),
        text(&output.stderr)
    );
    assert_eq!(text(&output.stdout), "");
    let stderr = text(&output.stderr);
    assert!(
        stderr.starts_with(&format!(
            "trust-lsp check: {}: not a source file of the project in ",
            outside.display()
        )),
        "stderr: {stderr}"
    );
}
