//! `trust-lsp command`: one project command for a tool, from the command line (specification 14 §7.8).
//!
//! The project is loaded as `trust-lsp check` loads it (§7.7). The command's one argument, a JSON object, comes on standard
//! input, and the answer of the handler that answers `workspace/executeCommand` goes to standard output as one line. No editor is
//! attached, so the argument's `open` texts must be empty, and the answer is the editor's own: an edit is returned, never applied.
//! Control Studio's engine uses it for the loop setup's catalog and for Keep in project's edit (specification 35, §0.1a, B1, B6.5).
//!
//! ```text
//! trust-lsp command <command> [--project DIR]
//! ```
//!
//! Exit status: 0 when the command answered (an answer with `"ok": false` included), 2 for a usage error, an unreadable argument
//! or an I/O error, with the reason on standard error.

use std::io::{Read, Write};
use std::path::PathBuf;

use serde_json::{json, Map, Value};
use tower_lsp::lsp_types::{ExecuteCommandParams, Url, WorkDoneProgressParams};

use crate::check::silent_client;
use crate::handlers;
use crate::state::ServerState;

/// The commands a command-line run answers: the editor's read-only project commands that answer facts or an edit (§7.8).
const COMMANDS: [&str; 3] = [
    "trust-lsp.hmiBindings",
    "trust-lsp.controlKeepParameters",
    "trust-lsp.hmiEquipment",
];

const USAGE: &str = "usage: trust-lsp command <command> [--project DIR]

Answers one project command for a tool, without an editor. The argument is one JSON object on
standard input; the answer is one JSON line on standard output.

  <command>       trust-lsp.hmiBindings, trust-lsp.controlKeepParameters or trust-lsp.hmiEquipment
  --project DIR   project root (default: .); the argument's root_uri defaults to this folder

Exit status: 0 when the command answered (an answer with \"ok\": false included), 2 usage or I/O error.
";

#[derive(Debug)]
struct Options {
    command: String,
    project: PathBuf,
}

fn parse_args(args: &[String]) -> Result<Options, String> {
    let mut command = None;
    let mut project = PathBuf::from(".");
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        match arg.as_str() {
            "--project" => {
                let value = iter.next().ok_or("--project needs a value")?;
                project = PathBuf::from(value);
            }
            "-h" | "--help" => return Err(String::new()),
            flag if flag.starts_with('-') => return Err(format!("unknown argument '{flag}'")),
            name if command.is_none() => command = Some(name.to_owned()),
            extra => return Err(format!("unexpected argument '{extra}'")),
        }
    }
    let command = command.ok_or("a command is required")?;
    if !COMMANDS.contains(&command.as_str()) {
        return Err(format!(
            "unknown command '{command}' ({})",
            COMMANDS.join(", ")
        ));
    }
    Ok(Options { command, project })
}

/// Runs `trust-lsp command` with the arguments after `command`, reading the argument from standard input; returns the exit status.
pub async fn run(args: &[String]) -> i32 {
    run_with_io(
        args,
        &mut std::io::stdin(),
        &mut std::io::stdout(),
        &mut std::io::stderr(),
    )
    .await
}

/// `run`, reading the argument from `input`, writing the answer to `out` and messages to `err`.
async fn run_with_io(
    args: &[String],
    input: &mut dyn Read,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> i32 {
    match command(args, input, out).await {
        Ok(status) => status,
        Err(message) => {
            // nothing more can be reported when stderr itself fails
            let _ = writeln!(err, "trust-lsp command: {message}");
            2
        }
    }
}

/// The command itself; `Err` is a usage or I/O problem (exit status 2).
async fn command(
    args: &[String],
    input: &mut dyn Read,
    out: &mut dyn Write,
) -> Result<i32, String> {
    let options = match parse_args(args) {
        Ok(options) => options,
        Err(message) if message.is_empty() => {
            write!(out, "{USAGE}").map_err(|error| format!("write output: {error}"))?;
            return Ok(0);
        }
        Err(message) => return Err(format!("{message}\n\n{USAGE}")),
    };
    let mut text = String::new();
    input
        .read_to_string(&mut text)
        .map_err(|error| format!("standard input: {error}"))?;
    let mut argument = argument_object(&text)?;
    let root = options
        .project
        .canonicalize()
        .map_err(|error| format!("{}: {error}", options.project.display()))?;
    let root_uri = Url::from_directory_path(&root)
        .map_err(|()| format!("{}: not a directory path", root.display()))?;
    argument
        .entry("root_uri")
        .or_insert_with(|| json!(root_uri.as_str()));

    let (client, _service) = silent_client();
    let state = ServerState::new();
    state.set_workspace_folders(vec![root_uri]);
    handlers::index_workspace(&client, &state).await;
    state.mark_index_first_pass_done();
    if state.documents().is_empty() {
        return Err(format!(
            "no source files found in {} (see include_paths in trust-lsp.toml)",
            root.display()
        ));
    }

    let params = ExecuteCommandParams {
        command: options.command.clone(),
        arguments: vec![Value::Object(argument)],
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    let answer = handlers::execute_read_command(&state, params)
        .ok_or_else(|| format!("{} gave no answer", options.command))?;
    writeln!(out, "{answer}").map_err(|error| format!("write output: {error}"))?;
    Ok(0)
}

/// The argument: one JSON object whose `open` texts are empty, because no editor is attached.
fn argument_object(text: &str) -> Result<Map<String, Value>, String> {
    let Value::Object(argument) = serde_json::from_str::<Value>(text)
        .map_err(|error| format!("the argument on standard input is not JSON: {error}"))?
    else {
        return Err("the argument on standard input is not a JSON object".to_owned());
    };
    match argument.get("open") {
        None => {}
        Some(Value::Object(texts)) if texts.is_empty() => {}
        Some(_) => {
            return Err("no editor is attached: the argument's `open` must be empty".to_owned())
        }
    }
    Ok(argument)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAIN: &str = "PROGRAM Main\nVAR\n    Parameters : PIDParameters := (ProportionalGain := LREAL#1.0, IntegralGain := LREAL#0.5);\nEND_VAR\nEND_PROGRAM\n";

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| (*s).to_owned()).collect()
    }

    /// A project of one source file, a hand-written record, and an equipment instance in `plant.toml`, in a fresh temporary folder
    /// that is removed on drop.
    struct Project(PathBuf);

    impl Project {
        fn new(name: &str) -> Self {
            let root = std::env::temp_dir()
                .join(format!("trust-lsp-command-{name}-{}", std::process::id()));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(root.join("src")).expect("src dir");
            std::fs::write(
                root.join("trust-lsp.toml"),
                "[project]\ninclude_paths = [\"src\"]\n",
            )
            .expect("config");
            std::fs::write(root.join("src/Main.st"), MAIN).expect("main");
            std::fs::write(
                root.join("plant.toml"),
                "format = 2\n\n[equipment.\"Main.TIC101\"]\nblock = \"Control.PID\"\n",
            )
            .expect("plant");
            Self(root)
        }

        fn root(&self) -> &str {
            self.0.to_str().expect("utf-8 path")
        }
    }

    impl Drop for Project {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    async fn command_output(arguments: &[&str], input: &str) -> (i32, String, String) {
        let mut out = Vec::new();
        let mut err = Vec::new();
        let status = run_with_io(&args(arguments), &mut input.as_bytes(), &mut out, &mut err).await;
        (
            status,
            String::from_utf8(out).expect("utf-8 stdout"),
            String::from_utf8(err).expect("utf-8 stderr"),
        )
    }

    #[test]
    fn parses_the_command_and_its_project() {
        let options = parse_args(&args(&["trust-lsp.hmiEquipment", "--project", "p"]))
            .expect("valid arguments");
        assert_eq!(options.command, "trust-lsp.hmiEquipment");
        assert_eq!(options.project, PathBuf::from("p"));
        assert!(parse_args(&args(&["trust-lsp.projectInfo"])).is_err());
        assert!(parse_args(&args(&["trust-lsp.hmiBindings", "extra"])).is_err());
        assert!(parse_args(&args(&["trust-lsp.hmiBindings", "--bogus"])).is_err());
        assert!(parse_args(&args(&["trust-lsp.hmiBindings", "--project"])).is_err());
    }

    #[tokio::test]
    async fn a_hand_written_record_answers_its_edit_on_one_line() {
        let project = Project::new("edit");
        // No `root_uri`: the project folder is the default.
        let input = json!({ "record": "Main.Parameters", "values": { "ProportionalGain": 2.5 } })
            .to_string();
        let (status, out, err) = command_output(
            &[
                "trust-lsp.controlKeepParameters",
                "--project",
                project.root(),
            ],
            &input,
        )
        .await;
        assert_eq!(status, 0, "stderr: {err}");
        assert_eq!(out.lines().count(), 1, "one JSON line: {out}");
        let answer: Value = serde_json::from_str(&out).expect("one JSON object");
        assert_eq!(answer["ok"], true);
        assert_eq!(answer["kept"], json!(["ProportionalGain"]));
        let (uri, edits) = answer["edit"]["changes"]
            .as_object()
            .expect("changes by file")
            .iter()
            .next()
            .expect("one changed file");
        assert!(uri.ends_with("/src/Main.st"), "{uri}");
        // The typed literal keeps its `LREAL#` prefix.
        assert_eq!(edits[0]["newText"], "LREAL#2.5");
        assert_eq!(answer["expected"][uri.as_str()], MAIN);
    }

    #[tokio::test]
    async fn hmi_equipment_describes_an_instance_of_the_project_plant_file() {
        let project = Project::new("equipment");
        let input = json!({ "instance": "Main.TIC101" }).to_string();
        let (status, out, err) = command_output(
            &["trust-lsp.hmiEquipment", "--project", project.root()],
            &input,
        )
        .await;
        assert_eq!(status, 0, "stderr: {err}");
        let answer: Value = serde_json::from_str(&out).expect("one JSON object");
        assert_eq!(answer["ok"], true);
        assert_eq!(answer["instance"], "Main.TIC101");
        // The block comes from `plant.toml`: the instance is not declared in PLC code.
        assert_eq!(answer["block"], "Control.PID");
        assert_eq!(answer["declared"], false);
        assert_eq!(answer["owned"], true);
    }

    #[tokio::test]
    async fn an_answer_that_refuses_is_still_an_answer() {
        let project = Project::new("refusal");
        let input =
            json!({ "record": "Main.Missing", "values": { "ProportionalGain": 1.0 } }).to_string();
        let (status, out, err) = command_output(
            &[
                "trust-lsp.controlKeepParameters",
                "--project",
                project.root(),
            ],
            &input,
        )
        .await;
        assert_eq!(status, 0, "stderr: {err}");
        let answer: Value = serde_json::from_str(&out).expect("one JSON object");
        assert_eq!(answer["ok"], false);
        assert!(answer["errors"][0]["message"]
            .as_str()
            .is_some_and(|message| message.contains("Main.Missing")));
    }

    #[tokio::test]
    async fn an_unknown_command_is_a_usage_error() {
        let project = Project::new("unknown");
        let (status, out, err) = command_output(
            &["trust-lsp.projectInfo", "--project", project.root()],
            "{}",
        )
        .await;
        assert_eq!(status, 2);
        assert_eq!(out, "");
        assert!(
            err.starts_with("trust-lsp command: unknown command 'trust-lsp.projectInfo'"),
            "stderr: {err}"
        );
    }

    #[tokio::test]
    async fn an_open_text_is_a_usage_error_because_no_editor_is_attached() {
        let project = Project::new("open");
        let input = json!({ "open": { "src/Main.st": MAIN }, "record": "Main.Parameters", "values": { "ProportionalGain": 2.5 } })
            .to_string();
        let (status, out, err) = command_output(
            &[
                "trust-lsp.controlKeepParameters",
                "--project",
                project.root(),
            ],
            &input,
        )
        .await;
        assert_eq!(status, 2);
        assert_eq!(out, "");
        assert!(err.contains("no editor is attached"), "stderr: {err}");
    }

    #[tokio::test]
    async fn an_argument_that_is_not_one_json_object_is_a_usage_error() {
        let project = Project::new("argument");
        for input in ["[1]", "", "{"] {
            let (status, out, err) = command_output(
                &["trust-lsp.hmiBindings", "--project", project.root()],
                input,
            )
            .await;
            assert_eq!(status, 2, "input {input:?}");
            assert_eq!(out, "");
            assert!(err.starts_with("trust-lsp command: "), "stderr: {err}");
        }
    }

    #[tokio::test]
    async fn help_prints_the_usage_and_succeeds() {
        let (status, out, err) = command_output(&["--help"], "").await;
        assert_eq!(status, 0);
        assert!(
            out.starts_with("usage: trust-lsp command <command>"),
            "{out}"
        );
        assert_eq!(err, "");
    }

    #[tokio::test]
    async fn a_project_without_source_files_is_a_usage_error() {
        let root =
            std::env::temp_dir().join(format!("trust-lsp-command-empty-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("empty project");
        let (status, out, err) = command_output(
            &[
                "trust-lsp.hmiBindings",
                "--project",
                root.to_str().expect("utf-8 path"),
            ],
            "{}",
        )
        .await;
        let _ = std::fs::remove_dir_all(&root);
        assert_eq!(status, 2);
        assert_eq!(out, "");
        assert!(
            err.starts_with("trust-lsp command: no source files found in "),
            "stderr: {err}"
        );
    }
}
