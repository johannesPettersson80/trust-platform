//! The credentials that cross the language server's channel (specification 25 §6.4): the request
//! fields that carry one, and the command answers that carry one. The `initialize` answer names
//! them (`capabilities.experimental.credentials`), and the extension refuses such a command while
//! its language client's trace logs each message's parameters, so that no trace logs a credential.
//! The language server itself never logs the messages it receives or sends.

use serde_json::{json, Value};

use super::project::SIMULATOR_CONTROL_COMMAND;

/// The request fields that carry a credential: a runtime's access token (run targets, HMI
/// deployment, a simulator journey's target), a control endpoint's token (Live Values), and a named
/// HMI session's cookie (a simulator journey).
const FIELDS: [&str; 3] = ["authToken", "controlAuthToken", "cookie"];

/// `capabilities.experimental.credentials` of the `initialize` answer: the fields, and the commands,
/// by their action, whose answer carries a credential. The simulator control's `create` answers a
/// new control token.
pub(crate) fn announcement() -> Value {
    json!({
        "fields": FIELDS,
        "answers": [{ "command": SIMULATOR_CONTROL_COMMAND, "action": "create" }],
    })
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
