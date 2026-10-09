//! The project's own symbols for the editor (specification 39, §14.4): the catalogue with them,
//! and the checks of a symbol's definition and drawing.

use std::path::Path;

use serde_json::{json, Value};
use trust_runtime::hmi::catalogue::Catalogue;
use trust_runtime::hmi::my_symbols::{parse_symbol, symbols_in, SYMBOLS_FOLDER};

use super::hmi_files::refused;
use super::{resolve_hmi_bindings_project_root, HmiBindingsCommandArgs, HMI_FILE_CHECK_COMMAND};
use crate::handlers::context::ServerContext;

/// The shipped catalogue with the project's symbols, `my:<name>`.
pub(super) fn project_catalogue(
    project: &Path,
    open: &std::collections::BTreeMap<std::path::PathBuf, String>,
) -> Result<Catalogue, String> {
    let bundle = super::hmi_authoring::bundle::read_edited(project, open)
        .map_err(|error| error.to_string())?;
    let (symbols, problems) = symbols_in(&bundle.files);
    if let Some(problem) = problems.first() {
        return Err(format!(
            "{}:{}: {}",
            problem.file, problem.line, problem.message
        ));
    }
    Ok(Catalogue::shipped().with_symbols(&symbols))
}

/// The problems of a symbol's definition, with its drawing from the project: a definition's on
/// their lines, a drawing's on the first line, naming the drawing's line.
pub(super) fn check_value<C: ServerContext>(
    context: &C,
    text: &str,
    file: &str,
    selection: &HmiBindingsCommandArgs,
) -> Value {
    let Some(project) = resolve_hmi_bindings_project_root(context, selection) else {
        return refused(format!(
            "unable to resolve workspace root for {HMI_FILE_CHECK_COMMAND}"
        ));
    };
    let name = Path::new(file)
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();
    let drawing_file = format!("{SYMBOLS_FOLDER}{name}.svg");
    let snapshot = match super::hmi_snapshot::paths(&project, &selection.open) {
        Ok(snapshot) => snapshot,
        Err(error) => return refused(error),
    };
    let bundle = match super::hmi_authoring::bundle::read_edited(&project, &snapshot) {
        Ok(bundle) => bundle,
        Err(error) => return refused(error.to_string()),
    };
    let Some(drawing) = bundle
        .files
        .get(&drawing_file)
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
    else {
        return json!({ "ok": true, "problems": [{ "line": 1, "message": format!("the symbol has no drawing ({drawing_file})") }] });
    };
    let problems: Vec<Value> = match parse_symbol(&name, text, drawing) {
        Ok(_) => Vec::new(),
        Err(errors) => errors
            .into_iter()
            .map(|error| {
                if error.file == drawing_file {
                    json!({ "line": 1, "message": format!("{drawing_file}, line {}: {}", error.line, error.message) })
                } else {
                    json!({ "line": error.line, "message": error.message })
                }
            })
            .collect(),
    };
    json!({ "ok": true, "problems": problems })
}
