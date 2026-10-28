//! `trust-runtime new`: creates a project folder with the files that a truST project starts from
//! (specification 25, Create Project). The checks and the files are in the library.

use std::path::{Path, PathBuf};

use serde::Serialize;
use trust_runtime::new_project::{create_project, NewProjectError};

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

/// Runs `trust-runtime new`. With `--json`, a refused or failed request prints the error object and
/// exits 1. Without it, the error is returned and printed with the usual `Error:` prefix.
pub fn run_new(parent: PathBuf, name: String, force: bool, json: bool) -> anyhow::Result<()> {
    match create_project(&parent, &name, force) {
        Ok(project) if json => {
            println!("{}", serde_json::to_string(&project_answer(&project))?);
            Ok(())
        }
        Ok(project) => {
            println!("Created project {}", project.display());
            Ok(())
        }
        Err(error) if json => {
            println!("{}", serde_json::to_string(&error_answer(&error))?);
            std::process::exit(1)
        }
        Err(error) => Err(error.into()),
    }
}

fn project_answer(project: &Path) -> ProjectAnswer {
    ProjectAnswer {
        project: project.display().to_string(),
        focus: project.join("src").join("Main.st").display().to_string(),
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
        let project = std::env::temp_dir().join("my-plc");
        let encoded = serde_json::to_string(&project_answer(&project)).expect("encode answer");
        let answer: serde_json::Value = serde_json::from_str(&encoded).expect("decode answer");
        let project_text = project.display().to_string();
        let focus_text = project.join("src").join("Main.st").display().to_string();
        assert_eq!(answer["project"].as_str(), Some(project_text.as_str()));
        assert_eq!(answer["focus"].as_str(), Some(focus_text.as_str()));
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
