//! The `inputs` verb (specification 35, §0.1b; A8.1 and B4.3): records one change of the loop's study inputs, as the page's
//! editors send it, and prints what the study then holds: the requirements Tune designs to, what the change invalidated and the
//! stages. The verb sends `controlStudio/inputs` through `ControlStudio::execute`, the entry that `serve` uses, so every rule is the
//! method's: which keys a change may name, how a requirements record is assembled and checked, and what a change invalidates. It
//! starts no operation and needs no runtime.

use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use clap::Args;
use serde_json::{json, Value};
use trust_control::json::parse_text;

use super::output::{items, member, text_field};
use super::{answer_of, follow, json_answer_of, places, text_of, Common, Exit, SELECTED_RUNTIME};
use crate::commands;
use crate::settings::Environment;

/// The arguments of `inputs` (§0.1b).
#[derive(Debug, Args)]
pub struct InputsArgs {
    /// The arguments that the verbs share.
    #[command(flatten)]
    pub common: Common,
    /// The loop whose study inputs change.
    #[arg(long = "loop", value_name = "ID")]
    pub loop_id: String,
    /// The JSON file of one change, as the page's editors send it, for example the requirements record that `referenceChange`
    /// marks; `-` reads it from standard input.
    #[arg(long, value_name = "FILE")]
    pub change: PathBuf,
}

/// Runs `inputs` (§0.1b). A change that cannot be read, or that is no JSON object, is a usage error (exit 2) with its reason on
/// standard error; a refusal of the method is answered as the other verbs answer it (exit 1), its reason under the field it names.
pub(super) fn run(
    args: &InputsArgs,
    environment: Environment<'_>,
    stdout: &mut dyn Write,
    stderr: &mut dyn Write,
) -> Exit {
    let as_json = args.common.json;
    let places = match places(&args.common, environment) {
        Ok(places) => places,
        Err(failure) => {
            return answer_of::<Value>(Err(failure), as_json, |_| String::new(), stdout, stderr)
        }
    };
    let change = match read_change(&args.change) {
        Ok(change) => change,
        Err(message) => {
            let _ = writeln!(stderr, "{message}");
            return Exit::Usage;
        }
    };
    let request = json!({
        "projectRoot": text_of(&places.project),
        "runtimeId": SELECTED_RUNTIME,
        "loopId": args.loop_id,
        "change": change,
        "storageFolder": text_of(&places.journal),
    });
    let (studio, _updates) = follow::engine();
    let Some(answer) = studio.execute(commands::INPUTS, &[request]) else {
        return Exit::Failed;
    };
    json_answer_of(&answer, as_json, inputs_text, stdout, stderr)
}

/// Reads the change as a JSON object, from `file` or, for `-`, from standard input. The text is read here; every rule about its
/// content is the method's.
fn read_change(file: &Path) -> Result<Value, String> {
    let (text, source) = if file == Path::new("-") {
        let mut text = String::new();
        std::io::stdin()
            .read_to_string(&mut text)
            .map_err(|error| format!("The change cannot be read from standard input: {error}."))?;
        (text, "from standard input".to_owned())
    } else {
        let text = fs::read_to_string(file)
            .map_err(|error| format!("The change {} cannot be read: {error}.", file.display()))?;
        (text, file.display().to_string())
    };
    match parse_text(&text) {
        Ok(change @ Value::Object(_)) => Ok(change),
        Ok(_) => Err(format!("The change {source} is not a JSON object.")),
        Err(error) => Err(format!("The change {source} is not JSON: {error}.")),
    }
}

/// The readable text of `inputs` (§0.1b): whether the change altered the study, the requirements the study now holds (each value the
/// engineer did not give marked Automatic), and the stages after the change.
fn inputs_text(result: &Value) -> String {
    let mut lines = vec![if result["changed"] == json!(true) {
        "Recorded.".to_owned()
    } else {
        "The study already held these inputs.".to_owned()
    }];
    let requirements = member(&result["inputs"], "requirements");
    if let Some(record) = requirements.as_object() {
        let automatic: Vec<&str> = items(&requirements["automaticCriteria"], "fields")
            .filter_map(Value::as_str)
            .collect();
        lines.push("Requirements:".to_owned());
        for (field, value) in record {
            if field == "automaticCriteria"
                || value.is_null()
                || value.is_object()
                || value.is_array()
            {
                continue;
            }
            let marker = if automatic.contains(&field.as_str()) {
                "  (Automatic)"
            } else {
                ""
            };
            lines.push(format!("  {field}: {value}{marker}"));
        }
    }
    lines.push("Stages:".to_owned());
    for stage in items(result, "stages") {
        let name = text_field(stage, "stage").unwrap_or_default();
        let label = text_field(stage, "label").unwrap_or_default();
        lines.push(format!("  {name}: {label}"));
    }
    lines.join("\n")
}
