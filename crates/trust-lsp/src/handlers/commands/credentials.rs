//! The credentials that cross the language server's channel (specification 25 §6.4): the request
//! fields that carry one, and the command answers that carry one. The `initialize` answer names
//! them (`capabilities.experimental.credentials`), and the extension refuses such a command while
//! its language client's trace logs each message's parameters, so that no trace logs a credential.
//! A field carries a credential when it holds non-blank text. The language server itself never logs
//! the messages it receives or sends.

use serde_json::{json, Value};

use trust_runtime::security::SECRET_PARAM_KEYS;

use super::project::SIMULATOR_CONTROL_COMMAND;

/// The fields of the channel's own requests that carry a credential: a runtime's access token (run
/// targets, HMI deployment, Devices & Connections, a simulator journey's target), a control
/// endpoint's token (Live Values), a named HMI session's cookie (a simulator journey), and a
/// project's `runtime.toml`, whose `[runtime.control]` and `[runtime.mesh]` hold tokens (the HMI
/// users).
const CHANNEL_FIELDS: [&str; 4] = ["authToken", "controlAuthToken", "cookie", "runtimeToml"];

/// `capabilities.experimental.credentials` of the `initialize` answer: the fields, the channel's
/// own and then the secret configuration keys, which a form or a runtime request sends by name
/// (specification 11 §6.9.7), and the commands, by their action, whose answer carries a
/// credential. The simulator control's `create` answers a new control token.
pub(crate) fn announcement() -> Value {
    let fields: Vec<&str> = CHANNEL_FIELDS
        .iter()
        .chain(SECRET_PARAM_KEYS.iter())
        .copied()
        .collect();
    json!({
        "fields": fields,
        "answers": [{ "command": SIMULATOR_CONTROL_COMMAND, "action": "create" }],
    })
}

#[cfg(test)]
#[path = "credentials_tests.rs"]
mod tests;
