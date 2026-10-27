//! The project commands at the protocol boundary (specification 25): the names the server advertises
//! and the extension must never share, and the refusal of malformed arguments. The rules are tested
//! beside each command's module.

use std::path::{Path, PathBuf};

use serde_json::{json, Value};
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::LanguageServer;

use super::*;
use crate::test_support::test_client;

fn repository() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[tokio::test]
async fn the_project_commands_are_advertised() {
    // The language client sends only commands the server advertises; a missing registration would
    // answer `workspace/executeCommand` but leave the extension unable to reach it.
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
    for command in PROJECT_COMMANDS {
        assert!(
            advertised.iter().any(|name| name == command),
            "{command} is advertised: {advertised:?}"
        );
    }
}

#[test]
fn the_project_commands_never_share_a_name_with_an_extension_command() {
    // The language client registers every advertised command as a VS Code command, so a name the
    // extension also registers stops the client from starting (specification 35, §0.1a).
    let extension = repository().join("editors/vscode");
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
    names.extend(
        manifest["activationEvents"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|event| {
                event
                    .as_str()?
                    .strip_prefix("onCommand:")
                    .map(str::to_owned)
            }),
    );
    collect_registrations(&extension.join("src"), &mut names);
    for command in PROJECT_COMMANDS {
        assert!(
            !names.iter().any(|name| name == command),
            "{command} is also an extension command"
        );
    }
}

/// The names of the `registerCommand("…")` calls below `folder`.
fn collect_registrations(folder: &Path, names: &mut Vec<String>) {
    let Ok(entries) = std::fs::read_dir(folder) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_registrations(&path, names);
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("ts" | "tsx")
        ) && let Ok(text) = std::fs::read_to_string(&path)
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

#[test]
fn a_project_command_takes_exactly_one_argument_object() {
    // Catches: a missing, extra or non-object argument read as defaults, which would let the
    // extension's wrong call pass silently.
    let state = ServerState::new();
    for arguments in [vec![], vec![json!({}), json!({})], vec![json!("text")]] {
        let answer = execute(&state, MANIFEST_CHECK_COMMAND, arguments);
        assert_eq!(answer["ok"], json!(false), "{answer}");
        assert_eq!(
            answer["error"],
            json!("trust-lsp.manifestCheck expects one argument object.")
        );
    }
    let answer = execute(&state, MANIFEST_CHECK_COMMAND, vec![json!({ "text": 1 })]);
    assert_eq!(answer["ok"], json!(false), "{answer}");
    assert!(
        answer["error"]
            .as_str()
            .is_some_and(|error| error.starts_with("Invalid arguments:")),
        "{answer}"
    );
}
