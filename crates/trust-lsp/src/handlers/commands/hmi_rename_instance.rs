//! Renaming equipment in the HMI Builder (specification 39, §21.3 item 2): the instance's
//! declaration is renamed as Rename Symbol renames it, so the program, the pages and `plant.toml`
//! follow in one edit that the editor applies. A PID loop's variables beside its PID carry its
//! name (§10.5): they are renamed with it, so its regions stay what Create PLC code writes.

use serde::Deserialize;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{
    RenameParams, TextDocumentIdentifier, TextDocumentPositionParams, Url, WorkDoneProgressParams,
    WorkspaceEdit,
};

use super::hmi_files::{refused, single_argument};
use super::{HmiBindingsCommandArgs, HMI_RENAME_INSTANCE_COMMAND};
use crate::handlers::lsp_utils::offset_to_position;
use crate::state::ServerState;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    root_uri: Url,
    /// The instance path, `Main.P101`.
    instance: String,
    new_name: String,
}

pub(super) fn rename_instance(state: &ServerState, args: Vec<Value>) -> Value {
    rename_result(state, args).unwrap_or_else(refused)
}

fn rename_result(state: &ServerState, args: Vec<Value>) -> Result<Value, String> {
    let args: Args = single_argument(args, HMI_RENAME_INSTANCE_COMMAND)?;
    let selection = HmiBindingsCommandArgs {
        open: Default::default(),
        root_uri: Some(args.root_uri),
        text_document: None,
    };
    let inputs = super::project_inputs(state, &selection, HMI_RENAME_INSTANCE_COMMAND)?;
    let mut edit = rename_declaration(state, &inputs, &args.instance, &args.new_name)?;
    let index = super::hmi_authoring::declarations::Declarations::read(&inputs.sources);
    let is_loop = index
        .resolve(&args.instance)
        .is_some_and(|node| super::hmi_generate::is_loop(&node.type_name));
    if let (true, Some((root, name))) = (is_loop, args.instance.split_once('.')) {
        let old = super::hmi_generate::loop_variable_names(name);
        let new = super::hmi_generate::loop_variable_names(&args.new_name);
        for (old, new) in old.into_iter().zip(new) {
            let path = format!("{root}.{old}");
            if super::hmi_generate::declaration_of(&inputs, &path).is_err() {
                continue;
            }
            let more = rename_declaration(state, &inputs, &path, &new)?;
            let changes = edit.changes.get_or_insert_with(Default::default);
            for (uri, edits) in more.changes.unwrap_or_default() {
                changes.entry(uri).or_default().extend(edits);
            }
        }
    }
    Ok(json!({ "ok": true, "edit": edit }))
}

/// The edit renaming the declaration of `instance` (`Main.P101`) as Rename Symbol does.
fn rename_declaration(
    state: &ServerState,
    inputs: &super::ProjectInputs,
    instance: &str,
    new_name: &str,
) -> Result<WorkspaceEdit, String> {
    let (file, offset) = super::hmi_generate::declaration_of(inputs, instance)?;
    let uri = Url::from_file_path(inputs.root.join(&file))
        .map_err(|()| format!("{file} is not a file"))?;
    let document = state
        .ensure_document(&uri)
        .ok_or_else(|| format!("{file} cannot be read"))?;
    let position = offset_to_position(
        &document.content,
        u32::try_from(offset).map_err(|_| format!("{file} is too large"))?,
    );
    let params = RenameParams {
        text_document_position: TextDocumentPositionParams {
            text_document: TextDocumentIdentifier { uri },
            position,
        },
        new_name: new_name.to_string(),
        work_done_progress_params: WorkDoneProgressParams::default(),
    };
    crate::handlers::rename(state, params)
        .map_err(|error| error.message.to_string())?
        .ok_or_else(|| format!("{instance} cannot be renamed"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::ProjectConfig;

    const PROGRAM: &str = "FUNCTION_BLOCK Pump\nVAR_OUTPUT\n    Running : BOOL;\nEND_VAR\nEND_FUNCTION_BLOCK\n\nPROGRAM Main\nVAR\n    P101 : Pump;\nEND_VAR\n    P101();\nEND_PROGRAM\n";
    const PAGE: &str = "format = 1\ntitle = \"Overview\"\n\n[[object]]\nid = \"pump\"\ntag = \"P101\"\ntype = \"status\"\nblock = \"Pump\"\ninstance = \"Main.P101\"\nat = [0, 0]\nsize = [40, 40]\n\n[object.bind]\nstate = \"Main.P101.Running\"\n";

    /// Specification 39, §21.3 item 2: renaming equipment in the builder renames its declaration,
    /// its calls and the page objects that show it, also while its program is not open.
    #[test]
    fn renaming_equipment_renames_the_program_and_its_page_objects() {
        let root = std::env::temp_dir().join(format!(
            "trustlsp-hmi-rename-instance-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("src");
        std::fs::create_dir_all(root.join("hmi/pages")).expect("pages");
        std::fs::write(root.join("src/main.st"), PROGRAM).expect("source");
        std::fs::write(root.join("hmi/pages/overview.toml"), PAGE).expect("page");
        let state = ServerState::new();
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        state.set_workspace_folders(vec![root_uri.clone()]);
        state.set_workspace_config(root_uri.clone(), ProjectConfig::load(&root));
        let answer = rename_instance(
            &state,
            vec![json!({ "root_uri": root_uri, "instance": "Main.P101", "new_name": "FeedPump" })],
        );
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let changes = answer["edit"]["changes"].as_object().expect("changes");
        let source = Url::from_file_path(root.join("src/main.st"))
            .expect("uri")
            .to_string();
        let page = Url::from_file_path(root.join("hmi/pages/overview.toml"))
            .expect("uri")
            .to_string();
        assert_eq!(
            changes[&source].as_array().expect("program edits").len(),
            2,
            "{answer}"
        );
        let page_texts: Vec<&str> = changes[&page]
            .as_array()
            .expect("page edits")
            .iter()
            .filter_map(|edit| edit["newText"].as_str())
            .collect();
        assert_eq!(
            page_texts,
            ["FeedPump", "FeedPump", "FeedPump"],
            "binding, instance and tag: {answer}"
        );
        let refused = rename_instance(
            &state,
            vec![json!({ "root_uri": root_uri, "instance": "Main.P999", "new_name": "X" })],
        );
        assert_eq!(refused["ok"], json!(false), "{refused}");
        std::fs::remove_dir_all(root).expect("remove fixture");
    }

    /// Specification 39, §10.5: renaming a PID loop renames its variables beside the PID too.
    #[test]
    fn renaming_a_loop_renames_its_variables_beside_the_pid() {
        // The loop's declarations stand first in their regions, right after the begin markers, as
        // Create PLC code writes them: a name is found by its code, not by the comment before it.
        let program = "NAMESPACE Control\nFUNCTION_BLOCK PID\nVAR_INPUT\n    Setpoint : LREAL;\nEND_VAR\nEND_FUNCTION_BLOCK\nEND_NAMESPACE\n\nPROGRAM Main\nVAR\n    TIC101X : BOOL;\nEND_VAR\nVAR\n    (* truST plant: declarations sha256:0 *)\n    TIC101 : Control.PID;\n    (* end of truST plant declarations *)\nEND_VAR\nVAR RETAIN\n    (* truST plant: retained declarations sha256:0 *)\n    TIC101_Setpoint : LREAL;\n    TIC101_Manual : BOOL;\n    (* end of truST plant retained declarations *)\nEND_VAR\n    TIC101(Setpoint := TIC101_Setpoint);\n    TIC101_Manual := TIC101X;\nEND_PROGRAM\n";
        let root =
            std::env::temp_dir().join(format!("trustlsp-hmi-rename-loop-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("src")).expect("src");
        std::fs::write(root.join("src/main.st"), program).expect("source");
        let state = ServerState::new();
        let root_uri = Url::from_directory_path(&root).expect("root uri");
        state.set_workspace_folders(vec![root_uri.clone()]);
        state.set_workspace_config(root_uri.clone(), ProjectConfig::load(&root));
        let answer = rename_instance(
            &state,
            vec![json!({ "root_uri": root_uri, "instance": "Main.TIC101", "new_name": "TIC105" })],
        );
        assert_eq!(answer["ok"], json!(true), "{answer}");
        let source = Url::from_file_path(root.join("src/main.st"))
            .expect("uri")
            .to_string();
        let mut texts: Vec<&str> = answer["edit"]["changes"][&source]
            .as_array()
            .expect("program edits")
            .iter()
            .filter_map(|edit| edit["newText"].as_str())
            .collect();
        texts.sort_unstable();
        assert_eq!(
            texts,
            [
                "TIC105",
                "TIC105",
                "TIC105_Manual",
                "TIC105_Manual",
                "TIC105_Setpoint",
                "TIC105_Setpoint"
            ],
            "the PID and its variables, declarations and uses; TIC101X is not the loop's: {answer}"
        );
        std::fs::remove_dir_all(root).expect("remove fixture");
    }
}
