//! The credentials that the `initialize` answer names (specification 25 §6.4).

use serde_json::{json, Value};
use tower_lsp::lsp_types::InitializeParams;
use tower_lsp::LanguageServer;

use crate::test_support::test_client;

/// The `initialize` answer's `capabilities.experimental`.
async fn experimental() -> Value {
    let server = crate::StLanguageServer::new(test_client());
    let result = server
        .initialize(InitializeParams::default())
        .await
        .expect("initialize");
    result
        .capabilities
        .experimental
        .expect("the experimental capabilities")
}

/// The `initialize` answer names the channel's credential fields, the secret configuration keys
/// and the one answer that carries a credential, as specification 25 §6.4 lists them. Catches a
/// credential that the extension would not refuse under a verbose trace because the language
/// server does not name it: Devices & Connections' runtime token (`token`) or a protocol form's
/// password (the review of #247), a project's `runtime.toml` text, or the simulator control's new
/// control token.
#[tokio::test]
async fn the_initialize_answer_names_the_credentials_that_cross_the_channel() {
    let experimental = experimental().await;
    assert_eq!(
        experimental["credentials"],
        json!({
            "fields": [
                "authToken", "controlAuthToken", "cookie", "runtimeToml",
                "password", "auth_token", "token", "credential", "credentials", "secret",
                "client_secret", "private_key"
            ],
            "answers": [{ "command": "trust-lsp.simulatorControl", "action": "create" }],
        })
    );
    assert_eq!(experimental["typeHierarchyProvider"], json!(true));
}

/// Every field that the communication setup schema marks secret is a named credential, since a form
/// sends a protocol's values by their field ids. Catches a new secret form field that the guard
/// would not know, such as one added to the schema without its key in the secret keys.
#[tokio::test]
async fn every_secret_field_of_the_setup_schema_is_a_named_credential() {
    let experimental = experimental().await;
    let named: Vec<&str> = experimental["credentials"]["fields"]
        .as_array()
        .expect("the fields")
        .iter()
        .filter_map(Value::as_str)
        .collect();
    let schema = trust_runtime::control::offline_comm_schema_json(None).expect("the schema");
    let secret: Vec<String> = schema["protocols"]
        .as_array()
        .expect("the protocols")
        .iter()
        .flat_map(|protocol| {
            let id = protocol["id"].as_str().unwrap_or_default().to_owned();
            protocol["fields"]
                .as_array()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|field| field["secret"] == true)
                .map(move |field| format!("{id}.{}", field["id"].as_str().unwrap_or_default()))
        })
        .collect();
    assert!(!secret.is_empty(), "the schema has secret fields");
    for field in &secret {
        let key = field.rsplit('.').next().unwrap_or_default();
        assert!(
            named.contains(&key),
            "{field} is a named credential: {named:?}"
        );
    }
}
