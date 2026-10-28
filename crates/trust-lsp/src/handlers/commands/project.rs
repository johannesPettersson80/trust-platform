//! The project commands of the language server (specification 25): the rules about a project's
//! files, its simulator entry, its libraries, its tests, the sidebar's gates and the examples
//! gallery. The extension sends what it holds (paths, settings, the diagnostics VS Code shows, the
//! user's choices) and shows what these commands answer.
//!
//! - `trust-lsp.runEntry` `{ action, … }`: the CONFIGURATION files a debug session can start from,
//!   the default CONFIGURATION and whether an entry names only PROGRAM types that the simulator's
//!   sources declare (§11.6).
//! - `trust-lsp.simulationSpeed` `{ action: "read" | "save", folder, speed?, target? }`: the
//!   project's saved simulation speed, and saving one (specification 46 §6.1 item 5).
//! - `trust-lsp.projectLibraries` `{ action, root, … }`: the Libraries panel's entries and edits (§7,
//!   §11.6).
//! - `trust-lsp.namespaceTarget` `{ action, … }`: the namespace under the cursor, a new namespace path
//!   and its default file (§1.1).
//! - `trust-lsp.testDiscovery` `{ action: "discover" | "project", … }`: the test declarations of
//!   Structured Text files and the project a test run uses (§11.1).
//! - `trust-lsp.compileGate` `{ diagnostics, compile?, target? }`: whether Start, Update and Debug may
//!   run (§6.6).
//! - `trust-lsp.projectRoot` `{ path?, folders, runtimeFolder? }`: the project that Compile checks
//!   (§6.2) and a workspace folder's runtime project.
//! - `trust-lsp.manifestCheck` `{ text }`: whether a trust-lsp.toml can be read (§6.2).
//! - `trust-lsp.examplesCatalog` `{ root, created? }` or `{ root, manifest, check: true }`: the
//!   examples gallery and its lessons, and the packaging check of its catalog (§3.1).
//! - `trust-lsp.copyExample` `{ action: "readme" | "name", … }`: a copied lesson's README links and a
//!   copy's folder name (§3.1).
//! - `trust-lsp.simulatorControl` `{ action: "create" | "check", … }`: the control endpoint and token
//!   of a launched simulator, and whether a launch may bind its endpoint (§6.8).
//!
//! Each answer is an object with `ok`; a refused request answers `ok: false` and an `error` text.

use serde::de::DeserializeOwned;
use serde_json::{json, Value};
use tower_lsp::lsp_types::{CodeActionOrCommand, CodeActionParams};

use crate::state::{Document, ServerState};

mod compile_gate;
mod examples;
pub(crate) mod libraries;
mod manifest_check;
mod namespace_target;
mod project_root;
mod run_entry;
mod simulation_speed;
mod simulator_control;
mod st_tests;

/// Whether a CONFIGURATION entry can run in the simulator.
pub const RUN_ENTRY_COMMAND: &str = "trust-lsp.runEntry";
/// The project's simulation speed.
pub const SIMULATION_SPEED_COMMAND: &str = "trust-lsp.simulationSpeed";
/// The Libraries panel.
pub const PROJECT_LIBRARIES_COMMAND: &str = "trust-lsp.projectLibraries";
/// The namespace move's target.
pub const NAMESPACE_TARGET_COMMAND: &str = "trust-lsp.namespaceTarget";
/// The Testing view's tests.
pub const TEST_DISCOVERY_COMMAND: &str = "trust-lsp.testDiscovery";
/// The sidebar's gates.
pub const COMPILE_GATE_COMMAND: &str = "trust-lsp.compileGate";
/// The project Compile checks.
pub const PROJECT_ROOT_COMMAND: &str = "trust-lsp.projectRoot";
/// Whether a project manifest can be read.
pub const MANIFEST_CHECK_COMMAND: &str = "trust-lsp.manifestCheck";
/// The examples gallery.
pub const EXAMPLES_CATALOG_COMMAND: &str = "trust-lsp.examplesCatalog";
/// A copied example.
pub const COPY_EXAMPLE_COMMAND: &str = "trust-lsp.copyExample";
/// A launched simulator's control endpoint.
pub const SIMULATOR_CONTROL_COMMAND: &str = "trust-lsp.simulatorControl";

/// The project commands, in the order the server advertises them. Each name is one camel-case
/// segment after `trust-lsp.`, so it never collides with an extension command such as
/// `trust-lsp.libraries.open`.
pub const PROJECT_COMMANDS: [&str; 11] = [
    RUN_ENTRY_COMMAND,
    SIMULATION_SPEED_COMMAND,
    PROJECT_LIBRARIES_COMMAND,
    NAMESPACE_TARGET_COMMAND,
    TEST_DISCOVERY_COMMAND,
    COMPILE_GATE_COMMAND,
    PROJECT_ROOT_COMMAND,
    MANIFEST_CHECK_COMMAND,
    EXAMPLES_CATALOG_COMMAND,
    COPY_EXAMPLE_COMMAND,
    SIMULATOR_CONTROL_COMMAND,
];

/// Whether `command` is a project command.
pub(super) fn is_command(command: &str) -> bool {
    PROJECT_COMMANDS.contains(&command)
}

/// The answer of the project command `command` to `arguments` (one object).
pub(super) fn execute(state: &ServerState, command: &str, arguments: Vec<Value>) -> Value {
    let Ok([argument]) = <[Value; 1]>::try_from(arguments) else {
        return refused(&format!("{command} expects one argument object."));
    };
    if !argument.is_object() {
        return refused(&format!("{command} expects one argument object."));
    }
    let answer = match command {
        RUN_ENTRY_COMMAND => run_entry::answer(state, argument),
        SIMULATION_SPEED_COMMAND => simulation_speed::answer(argument),
        PROJECT_LIBRARIES_COMMAND => libraries::answer(argument),
        NAMESPACE_TARGET_COMMAND => namespace_target::answer(state, argument),
        TEST_DISCOVERY_COMMAND => st_tests::answer(state, argument),
        COMPILE_GATE_COMMAND => compile_gate::answer(argument),
        PROJECT_ROOT_COMMAND => project_root::answer(argument),
        MANIFEST_CHECK_COMMAND => manifest_check::answer(argument),
        EXAMPLES_CATALOG_COMMAND => examples::catalog_answer(argument),
        COPY_EXAMPLE_COMMAND => examples::copy_answer(argument),
        SIMULATOR_CONTROL_COMMAND => simulator_control::answer(argument),
        _ => Err(format!("{command} is not a project command.")),
    };
    answer.unwrap_or_else(|message| refused(&message))
}

/// The project code actions at `params.range` of `doc`: adding a curated library whose symbol is
/// under the cursor (§7), and moving the namespace of a `NAMESPACE` or `USING` line (§1.1).
pub(crate) fn code_actions(
    state: &ServerState,
    doc: &Document,
    params: &CodeActionParams,
) -> Vec<CodeActionOrCommand> {
    let mut actions = libraries::code_actions(state, doc, params);
    actions.extend(namespace_target::code_actions(doc, params));
    actions
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
