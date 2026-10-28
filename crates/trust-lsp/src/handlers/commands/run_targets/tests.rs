//! The run-target commands at the protocol boundary: the names the server advertises and the
//! extension must never share, and the refusal of malformed arguments. The rules are tested beside
//! each module.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::LanguageServer;

use super::*;
use crate::test_support::test_client;

/// The language client sends only commands the server advertises. Catches a command that answers
/// `workspace/executeCommand` but that the extension cannot reach.
#[tokio::test]
async fn the_run_target_commands_are_advertised() {
    let server = crate::StLanguageServer::new(test_client());
    let result = server
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    let advertised = result
        .capabilities
        .execute_command_provider
        .expect("an executeCommand provider")
        .commands;
    for command in RUN_TARGET_COMMANDS {
        assert!(
            advertised.iter().any(|name| name == command),
            "{command} is advertised: {advertised:?}"
        );
    }
}

/// The language client registers every advertised command as a VS Code command, so a name the
/// extension also registers stops the client from starting. Catches such a collision.
#[test]
fn the_run_target_commands_never_share_a_name_with_an_extension_command() {
    let extension = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../editors/vscode");
    let manifest: Value = serde_json::from_str(
        &std::fs::read_to_string(extension.join("package.json")).expect("the extension's manifest"),
    )
    .expect("the manifest is JSON");
    let mut names: Vec<String> = manifest["contributes"]["commands"]
        .as_array()
        .expect("contributed commands")
        .iter()
        .filter_map(|command| command["command"].as_str().map(str::to_owned))
        .collect();
    registrations(&extension.join("src"), &mut names);
    for command in RUN_TARGET_COMMANDS {
        assert!(
            !names.iter().any(|name| name == command),
            "{command} is also an extension command"
        );
    }
}

/// The names of the `registerCommand("…")` calls below `folder`.
fn registrations(folder: &Path, names: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            registrations(&path, names);
        } else if path.extension().is_some_and(|ext| ext == "ts")
            && let Ok(text) = std::fs::read_to_string(&path)
        {
            let mut rest = text.as_str();
            while let Some(at) = rest.find("registerCommand(") {
                rest = &rest[at + "registerCommand(".len()..];
                if let Some(quoted) = rest.trim_start().strip_prefix('"')
                    && let Some(end) = quoted.find('"')
                {
                    names.push(quoted[..end].to_owned());
                }
            }
        }
    }
}

/// A missing, extra or non-object argument, a missing or unknown action and malformed fields are
/// refused with their reason. Catches a wrong call of the extension read as defaults.
#[test]
fn malformed_requests_are_refused() {
    for arguments in [vec![], vec![json!({}), json!({})], vec![json!("text")]] {
        let answer = execute(RUN_TARGETS_COMMAND, arguments);
        assert_eq!(answer["ok"], false, "{answer}");
        assert_eq!(
            answer["error"],
            "trust-lsp.runTargets expects one argument object."
        );
    }
    let answer = execute(RUN_TARGETS_COMMAND, vec![json!({})]);
    assert_eq!(answer["error"], "trust-lsp.runTargets needs an action.");
    let answer = execute(RUN_TARGETS_COMMAND, vec![json!({ "action": "launch" })]);
    assert_eq!(
        answer["error"],
        "trust-lsp.runTargets has no action launch."
    );
    let answer = execute(
        RUNTIME_REQUEST_COMMAND,
        vec![json!({ "action": "request" })],
    );
    assert_eq!(answer["ok"], false);
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("Invalid arguments:")),
        "{answer}"
    );
}
