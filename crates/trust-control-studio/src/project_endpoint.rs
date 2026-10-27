//! The endpoint and token of a project's own runtime, as `trust-runtime ctl --project <folder>
//! endpoint --json` reports them (specification 11 §6.8.0a; specification 35, §0.1b). trust-runtime
//! decides where the project's runtime listens: the endpoint file of the running runtime, else the
//! endpoint of the project's `runtime.toml`, else the platform default. The engine only runs that
//! command and reads its answer, as the VS Code extension does, so the rule has one owner.

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::json;

/// The other way to name the runtime, which every failure here offers.
const ENDPOINT_OPTION: &str = "pass --endpoint tcp://host:port or unix:///path";

/// The project's runtime as trust-runtime reports it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectEndpoint {
    /// The control endpoint.
    pub endpoint: String,
    /// The token that the project's runtime takes, when it has one: the token that it generated for
    /// its start, or the one that its `runtime.toml` configures.
    pub auth_token: Option<String>,
}

/// The `trust-runtime` executable that reports a project's endpoint.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimeTool {
    executable: PathBuf,
}

impl RuntimeTool {
    /// The tool at `executable`; without one, the `trust-runtime` beside this program, else the
    /// `trust-runtime` on the PATH.
    #[must_use]
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            executable: executable.unwrap_or_else(default_executable),
        }
    }

    /// The endpoint and the token of the runtime of the project at `project`.
    ///
    /// # Errors
    /// trust-runtime cannot run, or it reports no endpoint, for example because the runtime is not
    /// running: the failure says why and names `--endpoint` as the other way to name the runtime.
    pub fn project_endpoint(&self, project: &Path) -> Result<ProjectEndpoint, Failure> {
        let output = started_once_free(|| {
            Command::new(&self.executable)
                .arg("ctl")
                .arg("--project")
                .arg(project)
                .arg("endpoint")
                .arg("--json")
                // The engine's token is `TRUST_CONTROL_TOKEN` or `--token-file` (§0.1b), so
                // trust-runtime's own token variable must not stand in for the project's token in the
                // report.
                .env_remove("TRUST_CTL_TOKEN")
                .stdin(Stdio::null())
                .output()
        })
        .map_err(|error| {
                not_found(&format!(
                    "trust-runtime cannot be started to find the project's runtime ({}: {error}); {ENDPOINT_OPTION}.",
                    self.executable.display()
                ))
            })?;
        let answer = json::parse_text(String::from_utf8_lossy(&output.stdout).trim()).ok();
        if let Some(message) = answer.as_ref().and_then(reported_failure) {
            return Err(not_found(&format!(
                "The project's runtime cannot be found ({message}); start it, or {ENDPOINT_OPTION}."
            )));
        }
        if let Some(endpoint) = answer.as_ref().and_then(|answer| text(&answer["endpoint"])) {
            return Ok(ProjectEndpoint {
                endpoint,
                auth_token: answer
                    .as_ref()
                    .and_then(|answer| text(&answer["auth_token"])),
            });
        }
        let stderr = String::from_utf8_lossy(&output.stderr).trim().to_owned();
        let detail = if stderr.is_empty() {
            output.status.to_string()
        } else {
            format!("{}: {stderr}", output.status)
        };
        Err(not_found(&format!(
            "trust-runtime gave no endpoint for the project ({detail}); {ENDPOINT_OPTION}."
        )))
    }
}

/// The message of an answer `{"error": {"code", "message"}}`.
fn reported_failure(answer: &Value) -> Option<String> {
    let error = answer.get("error")?;
    Some(text(&error["message"]).unwrap_or_else(|| "no reason given".to_string()))
}

/// A nonblank string value.
fn text(value: &Value) -> Option<String> {
    value
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

fn not_found(message: &str) -> Failure {
    Failure::new(message, Cause::Settings, NextAction::ConnectRuntime)
}

/// The `trust-runtime` beside this program when it exists, else the name that the PATH resolves.
fn default_executable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|engine| engine.parent().map(|folder| folder.join(tool_file_name())))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(tool_file_name()))
}

fn tool_file_name() -> String {
    format!("trust-runtime{}", std::env::consts::EXE_SUFFIX)
}

/// The output of `run`, started again while the executable is still open for writing in another
/// process (`ETXTBSY`), as for a moment after it was written or replaced: the system starts it once
/// every writer has closed it.
fn started_once_free(
    mut run: impl FnMut() -> std::io::Result<std::process::Output>,
) -> std::io::Result<std::process::Output> {
    let mut attempts = 0;
    loop {
        match run() {
            Err(error)
                if error.kind() == std::io::ErrorKind::ExecutableFileBusy && attempts < 10 =>
            {
                attempts += 1;
                std::thread::sleep(std::time::Duration::from_millis(20));
            }
            outcome => return outcome,
        }
    }
}

/// A stand-in for trust-runtime in `folder`: a shell script that records its arguments in
/// `folder/arguments`, prints `answer` and exits with `status`.
#[cfg(all(test, unix))]
pub(crate) fn stub_tool(folder: &Path, answer: &str, status: i32) -> RuntimeTool {
    use std::os::unix::fs::PermissionsExt;

    let script = folder.join("trust-runtime-stub");
    let arguments = folder.join("arguments");
    std::fs::write(
        &script,
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$@\" > '{}'\ncat <<'ANSWER'\n{answer}\nANSWER\nexit {status}\n",
            arguments.display()
        ),
    )
    .expect("write the stand-in for trust-runtime");
    std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755))
        .expect("make the stand-in executable");
    RuntimeTool::new(Some(script))
}

#[cfg(test)]
#[path = "project_endpoint_tests.rs"]
mod tests;
