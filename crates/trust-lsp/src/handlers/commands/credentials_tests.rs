//! The credentials that the `initialize` answer names (specification 25 §6.4).

use serde_json::json;
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::LanguageServer;

use crate::test_support::test_client;

/// The `initialize` answer names the three request fields that carry a credential and the one
/// answer that carries one, as specification 25 §6.4 lists them. Catches a credential that the
/// extension would not refuse under a verbose trace because the language server does not name it,
/// such as the simulator target's token under its old name `token`, or the simulator control's new
/// control token.
#[tokio::test]
async fn the_initialize_answer_names_the_credentials_that_cross_the_channel() {
    let server = crate::StLanguageServer::new(test_client());
    let result = server
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    let experimental = result
        .capabilities
        .experimental
        .expect("the experimental capabilities");
    assert_eq!(
        experimental["credentials"],
        json!({
            "fields": ["authToken", "controlAuthToken", "cookie"],
            "answers": [{ "command": "trust-lsp.simulatorControl", "action": "create" }],
        })
    );
    assert_eq!(experimental["typeHierarchyProvider"], json!(true));
}
