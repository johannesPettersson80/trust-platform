use tracing::info;
use trust_debug::{DebugAdapter, DebugSession};
use trust_runtime::RuntimeBuilder;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .with_writer(std::io::stderr)
        .init();

    info!("Starting trust-debug adapter");
    let runtime = match RuntimeBuilder::new().build() {
        Ok(runtime) => runtime,
        Err(error) => {
            eprintln!("trust-debug initialization failed: {error}");
            std::process::exit(1);
        }
    };
    let session = DebugSession::new(runtime);
    let mut adapter = DebugAdapter::new(session);
    if let Err(err) = adapter.run_stdio() {
        eprintln!("trust-debug error: {err}");
        std::process::exit(1);
    }
}
