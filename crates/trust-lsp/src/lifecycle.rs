//! The language server's lifecycle (specification 14 §7.9): whether the client asked for `shutdown`
//! before `exit`, which decides the process's exit status, and the service that ends the process
//! when `exit` arrives.

use std::sync::atomic::{AtomicBool, Ordering};
use std::task::{Context, Poll};

use tower::Service;
use tower_lsp::jsonrpc::{Request, Response};

static SHUTDOWN_REQUESTED: AtomicBool = AtomicBool::new(false);

/// Records the client's `shutdown` request.
pub(crate) fn record_shutdown() {
    SHUTDOWN_REQUESTED.store(true, Ordering::SeqCst);
}

/// The exit status after `exit` (LSP 3.17, "Exit Notification"): `0` when `shutdown` came first,
/// `1` otherwise.
#[must_use]
pub fn exit_code() -> i32 {
    if SHUTDOWN_REQUESTED.load(Ordering::SeqCst) {
        0
    } else {
        1
    }
}

/// The language service `S`, which ends the process with [`exit_code`] when the client's `exit`
/// arrives. tower-lsp's server stops only at the next message or at the end of standard input,
/// and a client may send nothing more and keep the pipe open.
pub struct ExitAtOnce<S>(pub S);

impl<S> Service<Request> for ExitAtOnce<S>
where
    S: Service<Request, Response = Option<Response>>,
{
    type Response = Option<Response>;
    type Error = S::Error;
    type Future = S::Future;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.0.poll_ready(context)
    }

    fn call(&mut self, request: Request) -> Self::Future {
        if request.method() == "exit" {
            std::process::exit(exit_code());
        }
        self.0.call(request)
    }
}
