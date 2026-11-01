//! The HMI Builder's project rules (specification 39): what the editor writes into a project and
//! what it checks before it writes. The extension asks, shows the answer and applies the files it
//! is given; it keeps no rule of its own (AGENTS.md).
//!
//! - `trust-lsp.hmiWriteRules`: the runtime display's write rules a page's controls need, added to
//!   `runtime.toml` (§12.2).
//! - `trust-lsp.hmiUsers`: the runtime display's users in `runtime.toml` (§12.8).
//! - `trust-lsp.hmiPages`: the Pages panel's order, names and visibility (§3.3 item 8, §4.6).
//! - `trust-lsp.hmiFiles`: new page and panel files, and the references a renamed file keeps
//!   (§3.1, §8.7).
//! - `trust-lsp.hmiNewScreen`: a screen layout and its first page from a template (§4.10).
//! - `trust-lsp.hmiScreenColour`: a project colour chosen or reset in `hmi/screen.toml` (§14.1
//!   item 2).
//! - `trust-lsp.hmiImportSymbol`: a project symbol from an SVG drawing (§14.4 item 5).
//! - `trust-lsp.hmiTranslation`: a text's translation in its language file (§14.5).
//! - `trust-lsp.hmiPidSync`: a page and its P&ID kept one, and a process screen from a P&ID (§18
//!   phase 4).
//! - `trust-lsp.hmiEquipmentEdit`: equipment bindings, instances and a loop's signal lines (§6
//!   item 5, §8.5).
//! - `trust-lsp.hmiVariablePicker`: what a binding is offered and what a dropped variable creates
//!   (§7.1).
//! - `trust-lsp.hmiClipboard`: copy and paste of objects (§3.3 item 8).
//! - `trust-lsp.hmiChecks`: the small checks of new pages, the PLC cycle, languages and settings.
//! - `trust-lsp.hmiDeployTarget`: the web server a deployment goes to (§16.2 item 8).
//! - `trust-lsp.hmiLibraries`: the bundled libraries an HMI needs, installed into the project.
//!
//! Each command takes one argument object and answers an object with `ok`; a refused request
//! answers `ok: false` and an `error` text.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};

mod checks;
mod clipboard;
mod colours;
mod deploy;
mod equipment;
mod files;
mod libraries;
mod pages;
mod pid;
mod runtime_toml;
mod screens;
mod symbols;
mod translations;
mod users;
mod variables;
mod write_rules;

/// The write rules a page needs.
pub const HMI_WRITE_RULES_COMMAND: &str = "trust-lsp.hmiWriteRules";
/// The runtime display's users.
pub const HMI_USERS_COMMAND: &str = "trust-lsp.hmiUsers";
/// The Pages panel.
pub const HMI_PAGES_COMMAND: &str = "trust-lsp.hmiPages";
/// New page and panel files, and renamed files' references.
pub const HMI_FILES_COMMAND: &str = "trust-lsp.hmiFiles";
/// New HMI Screen.
pub const HMI_NEW_SCREEN_COMMAND: &str = "trust-lsp.hmiNewScreen";
/// A project colour.
pub const HMI_SCREEN_COLOUR_COMMAND: &str = "trust-lsp.hmiScreenColour";
/// Import Symbol.
pub const HMI_IMPORT_SYMBOL_COMMAND: &str = "trust-lsp.hmiImportSymbol";
/// A translation.
pub const HMI_TRANSLATION_COMMAND: &str = "trust-lsp.hmiTranslation";
/// A page and its P&ID.
pub const HMI_PID_SYNC_COMMAND: &str = "trust-lsp.hmiPidSync";
/// Equipment objects.
pub const HMI_EQUIPMENT_EDIT_COMMAND: &str = "trust-lsp.hmiEquipmentEdit";
/// The variable picker.
pub const HMI_VARIABLE_PICKER_COMMAND: &str = "trust-lsp.hmiVariablePicker";
/// Copy and paste.
pub const HMI_CLIPBOARD_COMMAND: &str = "trust-lsp.hmiClipboard";
/// The small checks.
pub const HMI_CHECKS_COMMAND: &str = "trust-lsp.hmiChecks";
/// The deployment's web server.
pub const HMI_DEPLOY_TARGET_COMMAND: &str = "trust-lsp.hmiDeployTarget";
/// The bundled libraries.
pub const HMI_LIBRARIES_COMMAND: &str = "trust-lsp.hmiLibraries";

/// The HMI Builder's commands, in the order the server advertises them. Each name is one
/// camel-case segment after `trust-lsp.`, so it never collides with an extension command such as
/// `trust-lsp.hmiBuilder.newPage`.
pub const HMI_BUILDER_COMMANDS: [&str; 15] = [
    HMI_WRITE_RULES_COMMAND,
    HMI_USERS_COMMAND,
    HMI_PAGES_COMMAND,
    HMI_FILES_COMMAND,
    HMI_NEW_SCREEN_COMMAND,
    HMI_SCREEN_COLOUR_COMMAND,
    HMI_IMPORT_SYMBOL_COMMAND,
    HMI_TRANSLATION_COMMAND,
    HMI_PID_SYNC_COMMAND,
    HMI_EQUIPMENT_EDIT_COMMAND,
    HMI_VARIABLE_PICKER_COMMAND,
    HMI_CLIPBOARD_COMMAND,
    HMI_CHECKS_COMMAND,
    HMI_DEPLOY_TARGET_COMMAND,
    HMI_LIBRARIES_COMMAND,
];

/// Whether `command` is an HMI Builder command.
pub(super) fn is_command(command: &str) -> bool {
    HMI_BUILDER_COMMANDS.contains(&command)
}

/// The answer of the HMI Builder command `command` to `arguments` (one object).
pub(super) fn execute(command: &str, arguments: Vec<Value>) -> Value {
    let Ok([argument]) = <[Value; 1]>::try_from(arguments) else {
        return refused(&format!("{command} expects one argument object."));
    };
    if !argument.is_object() {
        return refused(&format!("{command} expects one argument object."));
    }
    let answer = match command {
        HMI_WRITE_RULES_COMMAND => write_rules::answer(argument),
        HMI_USERS_COMMAND => users::answer(argument),
        HMI_PAGES_COMMAND => pages::answer(argument),
        HMI_FILES_COMMAND => files::answer(argument),
        HMI_NEW_SCREEN_COMMAND => screens::answer(argument),
        HMI_SCREEN_COLOUR_COMMAND => colours::answer(argument),
        HMI_IMPORT_SYMBOL_COMMAND => symbols::answer(argument),
        HMI_TRANSLATION_COMMAND => translations::answer(argument),
        HMI_PID_SYNC_COMMAND => pid::answer(argument),
        HMI_EQUIPMENT_EDIT_COMMAND => equipment::answer(argument),
        HMI_VARIABLE_PICKER_COMMAND => variables::answer(argument),
        HMI_CLIPBOARD_COMMAND => clipboard::answer(argument),
        HMI_CHECKS_COMMAND => checks::answer(argument),
        HMI_DEPLOY_TARGET_COMMAND => deploy::answer(argument),
        HMI_LIBRARIES_COMMAND => libraries::answer(argument),
        _ => Err(format!("{command} is not an HMI Builder command.")),
    };
    answer.unwrap_or_else(|message| refused(&message))
}

/// The arguments of a command, decoded into `T`.
fn decode<T: DeserializeOwned>(argument: Value) -> Result<T, String> {
    serde_json::from_value(argument).map_err(|error| format!("Invalid arguments: {error}."))
}

/// A refused request.
fn refused(message: &str) -> Value {
    json!({ "ok": false, "error": message })
}

#[cfg(test)]
mod tests;
