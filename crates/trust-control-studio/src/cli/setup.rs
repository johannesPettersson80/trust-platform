//! The `setup` verb (specification 35, §0.1b; B1.3): without `--draft`, prints what the compiled project offers a new loop, each
//! controller with what its code states and the structure setup proposes; with `--draft <file>`, checks the confirmed choices of
//! the draft, writes the loop definitions to `control-loops/` (all or none, never over an existing definition) and prints their
//! ids. The verb sends `controlStudio/setupDiscover` or `controlStudio/setupSave` through `ControlStudio::execute`, the entry that
//! `serve` uses, so every rule is the method's. It compiles the project through `trust-lsp command` and writes nothing else; it
//! needs no runtime.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use clap::Args;

use serde_json::{json, Value};
use trust_control::json::parse_text;

use super::output::{items, member, text_field};
use super::{answer_of, follow, json_answer_of, places, text_of, Common, Exit, SELECTED_RUNTIME};
use crate::commands;
use crate::settings::Environment;

/// The arguments of `setup` (§0.1b; B1.3). It names the project and no runtime: setup compiles the project and writes loop files.
#[derive(Debug, Args)]
pub struct SetupArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The JSON draft of the confirmed choices, as the setup form writes it. Without it the verb only shows what the project
    /// offers.
    #[arg(long, value_name = "FILE")]
    pub draft: Option<PathBuf>,
}

/// Runs `setup` (§0.1b). An unreadable draft, or one that is not JSON, is a usage error (exit 2) with its reason on standard
/// error; a refusal or failure of the method is answered as the other verbs answer it (exit 1).
pub(super) fn run(
    args: &SetupArgs,
    environment: Environment<'_>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let as_json = args.common.json;
    let project = match places(&args.common, environment) {
        Ok(places) => text_of(&places.project),
        Err(failure) => {
            return answer_of::<Value>(Err(failure), as_json, |_| String::new(), stdout, stderr)
        }
    };
    let mut request =
        json!({ "projectRoot": project, "runtimeId": SELECTED_RUNTIME, "languageTool": null });
    let (method, text): (&str, fn(&Value) -> String) = match &args.draft {
        None => (commands::SETUP_DISCOVER, setup_discover_text),
        Some(file) => match read_draft(file) {
            Ok(draft) => {
                request["draft"] = draft;
                (commands::SETUP_SAVE, setup_save_text)
            }
            Err(message) => {
                let _ = writeln!(stderr, "{message}");
                return Exit::Usage;
            }
        },
    };
    let (studio, _updates) = follow::engine();
    let Some(answer) = studio.execute(method, &[request]) else {
        return Exit::Failed;
    };
    json_answer_of(&answer, as_json, text, stdout, stderr)
}

/// Reads the draft file as JSON (B1.3). The file is read here; every rule about its content is the method's.
fn read_draft(file: &Path) -> Result<Value, String> {
    let text = fs::read_to_string(file)
        .map_err(|error| format!("The draft {} cannot be read: {error}.", file.display()))?;
    parse_text(&text).map_err(|error| format!("The draft {} is not JSON: {error}.", file.display()))
}

/// The readable text of `setup` without a draft (§0.1b; B1.3): the structure setup proposes, what the code states for each
/// controller, the loops already configured, and each rejected definition with its reason.
fn setup_discover_text(result: &Value) -> String {
    let form = member(result, "formModel");
    let mut lines = vec![format!(
        "Structure proposed: {}.",
        text_field(form, "structure").unwrap_or("identify")
    )];
    for prefill in items(form, "prefills") {
        lines.push(format!(
            "Controller {}:",
            text_field(prefill, "instance").unwrap_or("?")
        ));
        for (label, key) in [
            ("parameter record", "parameterRecord"),
            ("measurement", "processValue"),
            ("output", "manipulatedVariable"),
            ("setpoint", "setpoint"),
            ("mode", "mode"),
            ("manual output", "manualOutput"),
            ("startup flag", "startupFlag"),
        ] {
            if let Some(name) = text_field(prefill, key) {
                lines.push(format!("  {label}: {name}"));
            }
        }
    }
    for entry in items(result, "loops") {
        lines.push(format!(
            "Configured loop {} ({}) in {}",
            text_field(entry, "loopId").unwrap_or(""),
            text_field(entry, "name").unwrap_or(""),
            text_field(entry, "configurationPath").unwrap_or(""),
        ));
    }
    for rejection in items(result, "rejected") {
        lines.push(format!(
            "Rejected {}: {}",
            text_field(rejection, "configurationPath").unwrap_or(""),
            text_field(rejection, "reason").unwrap_or(""),
        ));
    }
    lines.join("\n")
}

/// The readable text of `setup` with a draft (§0.1b): the identifier and the file of each loop it wrote.
fn setup_save_text(result: &Value) -> String {
    let saved: Vec<&Value> = items(result, "loops").collect();
    let mut lines = vec![format!("Wrote {} loop(s).", saved.len())];
    for entry in saved {
        lines.push(format!(
            "{} {} in {}",
            text_field(entry, "id").unwrap_or(""),
            text_field(entry, "name").unwrap_or(""),
            text_field(entry, "file").unwrap_or(""),
        ));
    }
    lines.join("\n")
}
