use tracing::info;
use trust_debug::{DebugAdapter, DebugSession};

fn main() {
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
