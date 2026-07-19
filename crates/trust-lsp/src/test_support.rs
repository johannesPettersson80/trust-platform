//! Test helpers shared across LSP unit tests.

use std::sync::{Arc, Mutex};
use tower_lsp::{Client, ClientSocket, LanguageServer, LspService};

pub(crate) struct DummyServer;

#[tower_lsp::async_trait]
impl LanguageServer for DummyServer {
    async fn initialize(
        &self,
        _: tower_lsp::lsp_types::InitializeParams,
    ) -> tower_lsp::jsonrpc::Result<tower_lsp::lsp_types::InitializeResult> {
        Ok(tower_lsp::lsp_types::InitializeResult::default())
    }

    async fn shutdown(&self) -> tower_lsp::jsonrpc::Result<()> {
        Ok(())
    }
}

pub(crate) fn test_client() -> Client {
    let (client, _service, socket) = test_client_with_socket();
    drop(socket);
    client
}

pub(crate) fn test_client_with_socket() -> (Client, LspService<DummyServer>, ClientSocket) {
    let captured = Arc::new(Mutex::new(None));
    let captured_clone = captured.clone();
    let (service, socket) = LspService::new(move |client| {
        *captured_clone.lock().expect("lock test client") = Some(client.clone());
        DummyServer
    });
    let client = captured
        .lock()
        .expect("lock test client")
        .take()
        .expect("test client");
    (client, service, socket)
}

pub(crate) async fn initialize_test_client(service: &mut LspService<DummyServer>) {
    let params = serde_json::to_value(tower_lsp::lsp_types::InitializeParams::default())
        .expect("serialize initialize params");
    let request = tower_lsp::jsonrpc::Request::build("initialize")
        .id(1_i64)
        .params(params)
        .finish();
    let response = tower::Service::call(service, request)
        .await
        .expect("initialize service call")
        .expect("initialize response");
    assert!(
        response.error().is_none(),
        "test LSP client initialization failed: {response:?}"
    );
}
