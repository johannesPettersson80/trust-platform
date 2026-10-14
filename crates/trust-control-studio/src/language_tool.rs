//! The language tool that Control Studio's engine runs for what only the compiled project knows: Keep in project's edit (specification
//! 35, B6.5) and loop setup's catalog of signals and controllers (B1). The engine starts `trust-lsp command <command> --project <root>`
//! (specification 14 §7.8), writes the argument as one JSON object on standard input and reads the answer, one JSON line, from
//! standard output. The language server takes no part in it.

use std::ffi::OsString;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

use serde_json::Value;
use trust_control::failure::{Cause, Failure, NextAction};
use trust_control::json;

/// How long one tool command may run: the tool compiles the project first (specification 35, §0.1a).
pub const DEADLINE: Duration = Duration::from_secs(120);

/// How often a running tool is checked against its deadline.
const POLL: Duration = Duration::from_millis(20);

/// A project command that the language tool answers. The workflows depend on this trait, so their tests run without a tool.
pub trait ProjectTool {
    /// `command` answered for the project at `project` with `argument`: the one JSON object that the tool wrote.
    ///
    /// # Errors
    /// The tool cannot run, does not answer within its deadline, exits with an error, or answers something that is not one JSON
    /// object on one line.
    fn answer(&self, command: &str, project: &Path, argument: &Value) -> Result<Value, Failure>;
}

/// The `trust-lsp` executable that answers the project commands.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LanguageTool {
    executable: PathBuf,
}

impl LanguageTool {
    /// The tool at `executable`; without one, the `trust-lsp` beside this program, else the `trust-lsp` on the PATH.
    #[must_use]
    pub fn new(executable: Option<PathBuf>) -> Self {
        Self {
            executable: executable.unwrap_or_else(default_executable),
        }
    }
}

impl ProjectTool for LanguageTool {
    fn answer(&self, command: &str, project: &Path, argument: &Value) -> Result<Value, Failure> {
        let arguments = [
            OsString::from("command"),
            OsString::from(command),
            OsString::from("--project"),
            project.as_os_str().to_owned(),
        ];
        run_tool(
            &self.executable,
            &arguments,
            &argument.to_string(),
            DEADLINE,
        )
    }
}

/// The `trust-lsp` beside this program when it exists, else the name that the PATH resolves.
fn default_executable() -> PathBuf {
    std::env::current_exe()
        .ok()
        .and_then(|engine| engine.parent().map(|folder| folder.join(tool_file_name())))
        .filter(|candidate| candidate.is_file())
        .unwrap_or_else(|| PathBuf::from(tool_file_name()))
}

fn tool_file_name() -> String {
    format!("trust-lsp{}", std::env::consts::EXE_SUFFIX)
}

/// Runs `executable` with `arguments`, `input` on standard input, and reads its answer: one JSON object on standard output. A run
/// that outlasts `deadline` is killed.
fn run_tool(
    executable: &Path,
    arguments: &[OsString],
    input: &str,
    deadline: Duration,
) -> Result<Value, Failure> {
    let mut child = Command::new(executable)
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|error| tool_failure(executable, &format!("it cannot be started: {error}")))?;
    if let Some(mut stdin) = child.stdin.take() {
        // Written beside the wait, so a tool that stops reading cannot hold the deadline.
        let input = input.to_owned();
        thread::spawn(move || {
            let _ = stdin.write_all(input.as_bytes());
        });
    }
    // Both pipes are drained while the tool runs, so a tool that writes a lot cannot block on a full pipe.
    let stdout_pipe = child.stdout.take();
    let stderr_pipe = child.stderr.take();
    let stdout_reader = thread::spawn(move || read_all(stdout_pipe));
    let stderr_reader = thread::spawn(move || read_all(stderr_pipe));

    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < deadline => thread::sleep(POLL),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(tool_failure(
                    executable,
                    &format!("it did not answer within {deadline:?}"),
                ));
            }
            Err(error) => {
                return Err(tool_failure(
                    executable,
                    &format!("its status cannot be read: {error}"),
                ))
            }
        }
    };
    let stdout = stdout_reader.join().unwrap_or_default();
    let stderr_bytes = stderr_reader.join().unwrap_or_default();
    let stderr = String::from_utf8_lossy(&stderr_bytes).trim().to_owned();
    if !status.success() {
        let detail = if stderr.is_empty() {
            String::new()
        } else {
            format!(": {stderr}")
        };
        return Err(tool_failure(
            executable,
            &format!("it exited with {status}{detail}"),
        ));
    }
    match json::parse_text(String::from_utf8_lossy(&stdout).trim()) {
        Ok(answer @ Value::Object(_)) => Ok(answer),
        _ => Err(tool_failure(
            executable,
            "its answer is not one JSON object on one line",
        )),
    }
}

/// What a pipe of a tool holds, read to its end.
fn read_all(pipe: Option<impl Read>) -> Vec<u8> {
    let mut bytes = Vec::new();
    if let Some(mut pipe) = pipe {
        let _ = pipe.read_to_end(&mut bytes);
    }
    bytes
}

fn tool_failure(executable: &Path, reason: &str) -> Failure {
    Failure::new(
        &format!(
            "The language tool {} did not answer: {reason}.",
            executable.display()
        ),
        Cause::Implementation,
        NextAction::None,
    )
}

#[cfg(test)]
#[path = "language_tool_tests.rs"]
mod tests;
