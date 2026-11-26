//! `trust-runtime new`: creates a project folder with the files that a truST project starts from
//! (specification 25, Create Project). The checks and the files are in the library.

use std::path::{Path, PathBuf};

use serde::Serialize;
use trust_runtime::new_project::{create_project_as, NewProjectError, ProjectStart};

/// The answer for a created project. The field order is the one the specification names.
#[derive(Debug, Serialize)]
struct ProjectAnswer {
    project: String,
    focus: String,
}

#[derive(Debug, Serialize)]
struct ErrorAnswer {
    error: ErrorBody,
}

#[derive(Debug, Serialize)]
struct ErrorBody {
    code: &'static str,
    message: String,
}

/// Runs `trust-runtime new` for the start `start`. With `--json`, a refused or failed request prints
/// the error object and exits 1. Without it, the error is returned and printed with the usual
/// `Error:` prefix.
pub fn run_new(
    parent: PathBuf,
    name: String,
    start: ProjectStart,
    force: bool,
    json: bool,
) -> anyhow::Result<()> {
    match create_project_as(&parent, &name, start, force) {
        Ok(project) if json => {
            outln!(
                "{}",
                serde_json::to_string(&project_answer(&project, start))?
            );
            Ok(())
        }
        Ok(project) => {
            outln!("Created project {}", project.display());
            Ok(())
        }
        Err(error) if json => {
            outln!("{}", serde_json::to_string(&error_answer(&error))?);
            std::process::exit(1)
        }
        Err(error) => Err(error.into()),
    }
}

/// The answer for `project`, created as `start`: the folder, and the file it opens on, its parts
/// joined as this computer joins them.
fn project_answer(project: &Path, start: ProjectStart) -> ProjectAnswer {
    let focus = start
        .focus()
        .split('/')
        .fold(project.to_path_buf(), |path, part| path.join(part));
    ProjectAnswer {
        project: project.display().to_string(),
        focus: focus.display().to_string(),
    }
}

fn error_answer(error: &NewProjectError) -> ErrorAnswer {
    ErrorAnswer {
        error: ErrorBody {
            code: error.code.as_str(),
            message: error.message.clone(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use trust_runtime::new_project::NewProjectErrorCode;

    #[test]
    fn the_answer_names_the_project_folder_and_its_main_program() {
        let project = trust_test_support::unique_temp_path("my-plc");
        let encoded =
            serde_json::to_string(&project_answer(&project, ProjectStart::StructuredText))
                .expect("encode answer");
        let answer: serde_json::Value = serde_json::from_str(&encoded).expect("decode answer");
        let project_text = project.display().to_string();
        let focus_text = project.join("src").join("Main.st").display().to_string();
        assert_eq!(answer["project"].as_str(), Some(project_text.as_str()));
        assert_eq!(answer["focus"].as_str(), Some(focus_text.as_str()));
    }

    #[test]
    fn the_answer_of_a_rust_start_names_the_crates_source() {
        // Specification 25 §3.2: a Rust start opens on its crate's source, `rust/src/lib.rs` with
        // `--rust` and `src/lib.rs` with `--rust-first`, its parts joined as this computer joins
        // them. Catches a Rust start that opens `src/Main.st`, which a Rust-first project lacks.
        let project = trust_test_support::unique_temp_path("my-cell");
        for (start, focus) in [
            (
                ProjectStart::StructuredTextWithRust,
                project.join("rust").join("src").join("lib.rs"),
            ),
            (ProjectStart::RustFirst, project.join("src").join("lib.rs")),
        ] {
            let encoded =
                serde_json::to_string(&project_answer(&project, start)).expect("encode answer");
            let answer: serde_json::Value = serde_json::from_str(&encoded).expect("decode answer");
            assert_eq!(
                answer["focus"].as_str(),
                Some(focus.display().to_string().as_str()),
                "{start:?}"
            );
        }
    }

    #[test]
    fn the_error_answer_names_the_code_and_the_message() {
        let error = NewProjectError {
            code: NewProjectErrorCode::Exists,
            message: "busy".to_string(),
        };
        let answer = serde_json::to_string(&error_answer(&error)).expect("encode answer");
        assert_eq!(answer, r#"{"error":{"code":"exists","message":"busy"}}"#);
    }
}
