//! Renaming a PLC instance keeps the equipment design in step (specification 39, §10.4): the
//! choices move to the new path and references follow, in the same workspace edit.

use super::{PlantDesign, PLANT_FILE};
use crate::handlers::{context::ServerContext, lsp_utils::offset_to_position};
use std::path::Path;
use tower_lsp::lsp_types::{Range, TextEdit, Url};
use trust_ide::rename_identity::RenameIdentity;

pub(crate) fn rename_edit<C: ServerContext>(
    context: &C,
    root: &Path,
    identity: &RenameIdentity,
    new_name: &str,
) -> Result<Option<(Url, TextEdit)>, String> {
    let path = root.join(PLANT_FILE);
    let uri = Url::from_file_path(&path).map_err(|_| "Invalid plant.toml path")?;
    let text = if let Some(document) = context.get_document(&uri) {
        document.content.clone()
    } else {
        match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(error) => return Err(format!("plant.toml cannot be read: {error}")),
        }
    };
    let mut design = PlantDesign::parse(&text)?;
    // Design paths name program instances; a member rename names the program's type.
    let selection = super::super::HmiBindingsCommandArgs {
        open: Default::default(),
        root_uri: Url::from_directory_path(root).ok(),
        text_document: None,
    };
    let configured = match super::super::project_inputs(context, &selection, "rename") {
        Ok(inputs) => {
            let texts: Vec<&str> = inputs
                .sources
                .iter()
                .map(|source| source.text.as_str())
                .collect();
            super::super::control_links::program_types(&texts)
        }
        Err(_) => Vec::new(),
    };
    let program_type = |instance: &str| {
        configured
            .iter()
            .find(|(name, _)| name.eq_ignore_ascii_case(instance))
            .map_or_else(|| instance.to_string(), |(_, kind)| kind.clone())
    };
    let mut renames = Vec::new();
    for path in design.equipment.keys() {
        let Some((root_name, member)) = path.split_once('.') else {
            continue;
        };
        match identity {
            RenameIdentity::Member { owner, name: old }
                if member.eq_ignore_ascii_case(old)
                    && program_type(root_name).eq_ignore_ascii_case(owner) =>
            {
                renames.push((path.clone(), format!("{root_name}.{new_name}")));
            }
            RenameIdentity::Root { name: old } if root_name.eq_ignore_ascii_case(old) => {
                renames.push((path.clone(), format!("{new_name}.{member}")));
            }
            // A program without a configuration entry is its own instance.
            RenameIdentity::Program { name: old }
                if root_name.eq_ignore_ascii_case(old)
                    && program_type(root_name).eq_ignore_ascii_case(old) =>
            {
                renames.push((path.clone(), format!("{new_name}.{member}")));
            }
            _ => {}
        }
    }
    if renames.is_empty() {
        return Ok(None);
    }
    for (from, to) in renames {
        design.rename_instance(&from, &to);
    }
    Ok(Some((
        uri,
        TextEdit {
            range: Range {
                start: offset_to_position(&text, 0),
                end: offset_to_position(&text, text.len() as u32),
            },
            new_text: design.write(),
        },
    )))
}
