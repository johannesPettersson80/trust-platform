//! The language server's side of Control Studio (specification 35, §0.1a): operation updates go
//! to the client as the `trust/controlStudio/update` notification.

use std::sync::Arc;

use serde_json::Value;
use tower_lsp::lsp_types::notification::Notification;
use tower_lsp::Client;
use trust_control::messages::OperationUpdate;
use trust_lsp::control_studio::commands::{ControlStudio, UPDATE_NOTIFICATION};
use trust_lsp::control_studio::operation::UpdateSink;

enum ControlStudioUpdate {}

impl Notification for ControlStudioUpdate {
    type Params = Value;
    const METHOD: &'static str = UPDATE_NOTIFICATION;
}

/// Sends operation updates to the client. Operations run on their own threads, so each update is
/// sent from the server's runtime, captured when the server starts.
struct ClientUpdates {
    client: Client,
    runtime: Option<tokio::runtime::Handle>,
}

impl UpdateSink for ClientUpdates {
    fn send(&self, update: OperationUpdate) {
        let Some(runtime) = &self.runtime else {
            tracing::warn!(
                "Control Studio update for {} dropped: no runtime to send it from",
                update.operation_id
            );
            return;
        };
        let params = match serde_json::to_value(&update) {
            Ok(params) => params,
            Err(error) => {
                tracing::warn!(
                    "Control Studio update for {} dropped: {error}",
                    update.operation_id
                );
                return;
            }
        };
        let client = self.client.clone();
        runtime.spawn(async move {
            client
                .send_notification::<ControlStudioUpdate>(params)
                .await
        });
    }
}

/// Control Studio for the server whose client is `client`.
pub fn new_control_studio(client: &Client) -> ControlStudio {
    ControlStudio::new(Arc::new(ClientUpdates {
        client: client.clone(),
        runtime: tokio::runtime::Handle::try_current().ok(),
    }))
}
