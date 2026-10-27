use tracing::info;
use trust_debug::{DebugAdapter, DebugSession};

fn main() {
    // ControlBridge searches supervise the current host executable. Dispatch
    // the shared bounded worker before starting DAP or its logging surface.
    if std::env::args().nth(1).as_deref() == Some("control-search-worker") {
        if let Err(error) = trust_runtime::control::search::worker_stdio() {
            eprintln!("control-search-worker: {error}");
            std::process::exit(1);
        }
        return;
    }

    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting trust-debug adapter");
    let mut adapter = DebugAdapter::new(DebugSession::new());
    if let Err(err) = adapter.run_stdio() {
        eprintln!("trust-debug error: {err}");
        std::process::exit(1);
    }
}
