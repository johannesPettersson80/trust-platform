//! Blockly programs, `*.blockly.json` (specification 25 §11.4, specification 17 §3.1).
//!
//! A Blockly program is the workspace the Blockly editor saves: its blocks (`blocks.blocks`), its
//! variables and its metadata. Its companion is one `FUNCTION_BLOCK` that runs the top-level block
//! chains in order, with the program's variables in `VAR` (an untyped variable takes the type of
//! the values assigned to it), and with every direct address lowered to a global
//! (`crate::direct_io`). Blocks the companion cannot express become warnings, listed in a comment
//! at the top of the companion and answered to the editor.
//!
//! The editor reads and writes the workspace itself (it is the Blockly library's own format); this
//! module owns what the workspace means as Structured Text.

mod blocks;
#[cfg(test)]
mod tests;

use crate::direct_io::with_external_globals;
use crate::naming::{comment_text, fb_name_for_source};
use crate::{Generated, Json, Problem, VisualKind};

/// The message when a file is not a Blockly workspace.
const FORMAT_MESSAGE: &str =
    "Invalid Blockly workspace format. Expected object with blocks.blocks[]";

/// Checks that `workspace` is a Blockly workspace: an object whose `blocks` member holds a
/// `blocks` array.
///
/// # Errors
/// The workspace has another shape; the message says which shape is expected.
pub fn check_format(workspace: &Json) -> Result<(), String> {
    let has_blocks = workspace
        .get("blocks")
        .and_then(|blocks| blocks.get("blocks"))
        .is_some_and(|items| items.as_array().is_some());
    if has_blocks {
        Ok(())
    } else {
        Err(FORMAT_MESSAGE.to_owned())
    }
}

/// The companion of the Blockly program `workspace` saved as `<base_name>.blockly.json`
/// (specification 17 §3).
///
/// The function block is called `FB_<base_name>_BLOCKLY`; without a base name the workspace's
/// `metadata.name` names it, and without that `Blockly`. Blocks the companion cannot express
/// (unknown block types, a chain that leads back to itself) are listed in `warnings` and in a
/// comment above the function block.
///
/// # Errors
/// The workspace is not a Blockly workspace ([`check_format`]), or its `variables` member is not
/// a list of variables.
pub fn generate(workspace: &Json, base_name: &str) -> Result<Generated, Vec<Problem>> {
    check_format(workspace).map_err(|message| vec![Problem::new(message)])?;
    let source = if base_name.is_empty() {
        workspace
            .get("metadata")
            .and_then(|metadata| metadata.str_at("name"))
            .filter(|name| !name.is_empty())
            .unwrap_or("Blockly")
    } else {
        base_name
    };
    let block_name = fb_name_for_source(source, VisualKind::Blockly.block_suffix());
    let conversion = blocks::convert(workspace).map_err(|problem| vec![problem])?;

    let mut lines = vec![format!("FUNCTION_BLOCK {block_name}"), String::new()];
    if !conversion.variables.is_empty() {
        lines.push("VAR".to_owned());
        for (name, ty) in &conversion.variables {
            lines.push(format!("  {name} : {ty};"));
        }
        lines.push("END_VAR".to_owned());
        lines.push(String::new());
    }
    lines.push("(* Generated from Blockly *)".to_owned());
    lines.push(String::new());
    lines.extend(conversion.body);
    lines.push(String::new());
    lines.push("END_FUNCTION_BLOCK".to_owned());

    let (function_block, globals) = with_external_globals(&lines.join("\n"));
    let function_block = if conversion.errors.is_empty() {
        function_block
    } else {
        format!("{}{function_block}", warnings_comment(&conversion.errors))
    };
    Ok(Generated {
        function_block,
        globals,
        warnings: conversion.errors,
    })
}

/// The comment listing the conversion warnings, followed by a blank line.
fn warnings_comment(errors: &[String]) -> String {
    let mut comment = format!("(*\n  Blockly conversion warnings ({}):\n", errors.len());
    for error in errors {
        comment.push_str("  - ");
        comment.push_str(&comment_text(error));
        comment.push('\n');
    }
    comment.push_str("*)\n\n");
    comment
}
