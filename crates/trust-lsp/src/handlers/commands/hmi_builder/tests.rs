//! The HMI Builder commands' shared request handling.

use serde_json::json;

use super::{execute, HMI_BUILDER_COMMANDS};

#[test]
fn every_command_takes_exactly_one_argument_object() {
    // A command given no argument, two arguments or a non-object would otherwise be read as an
    // empty request and answer as if the user had asked for nothing.
    for command in HMI_BUILDER_COMMANDS {
        for arguments in [vec![], vec![json!({}), json!({})], vec![json!("text")]] {
            let answer = execute(command, arguments);
            assert_eq!(answer["ok"], json!(false), "{command}");
            assert_eq!(
                answer["error"],
                json!(format!("{command} expects one argument object.")),
                "{command}"
            );
        }
    }
}

#[test]
fn an_unknown_command_is_refused() {
    let answer = execute("trust-lsp.hmiNothing", vec![json!({})]);
    assert_eq!(
        answer,
        json!({"ok": false, "error": "trust-lsp.hmiNothing is not an HMI Builder command."})
    );
}

#[test]
fn every_command_is_one_camel_case_segment_after_trust_lsp() {
    // A name with a second dot would collide with the extension's own `trust-lsp.hmiBuilder.*`
    // commands, which the language client refuses to register twice.
    for command in HMI_BUILDER_COMMANDS {
        let rest = command.strip_prefix("trust-lsp.").expect(command);
        assert!(!rest.contains('.') && rest.starts_with("hmi"), "{command}");
    }
}
