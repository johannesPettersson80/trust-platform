//! `trust-lsp` - Language Server Protocol implementation for IEC 61131-3 Structured Text.
//!
//! This is the main entry point for the ST language server.

use tower_lsp::{LspService, Server};
use tracing::info;
use tracing_subscriber::layer::SubscriberExt as _;
use tracing_subscriber::util::SubscriberInitExt as _;
use trust_lsp::{check, command_line, hmi_agent_mcp, StLanguageServer};

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    let command_line_check = args.get(1).map(String::as_str) == Some("check");
    let command_line_command = args.get(1).map(String::as_str) == Some("command");
    let command_line_run = command_line_check || command_line_command;
    // Initialize logging (command-line runs only report problems)
    let level = if command_line_run {
        tracing::Level::WARN
    } else {
        tracing::Level::INFO
    };
    let mut filter = tracing_subscriber::EnvFilter::from_default_env().add_directive(level.into());
    if command_line_run {
        // a command-line run's client has no peer: its log/progress notifications are dropped on purpose
        if let Ok(directive) = "tower_lsp::service::client=off".parse() {
            filter = filter.add_directive(directive);
        }
    }
    // The language server never logs the messages it receives or sends: they carry credentials,
    // such as an HMI agent simulator journey's named-session cookie (specification 43, AT-08).
    // tower-lsp traces each of them under `tower_lsp::codec`; this filter drops that target whatever
    // `RUST_LOG` says, where a more specific directive (`tower_lsp::codec[{message}]=trace`) would
    // outrank an `off` directive.
    let silent_transport = tracing_subscriber::filter::filter_fn(|metadata| {
        !metadata.target().starts_with("tower_lsp::codec")
    });
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_writer(std::io::stderr)
        .finish()
        .with(silent_transport)
        .init();

    if command_line_check {
        std::process::exit(check::run(&args[2..]).await);
    }
    if command_line_command {
        std::process::exit(command_line::run(&args[2..]).await);
    }
    // The HMI authoring tools, served to an agent over MCP (specification 43, §3.1).
    if args.get(1).map(String::as_str) == Some("hmi-agent") {
        std::process::exit(hmi_agent_mcp::run(&args[2..]));
    }

    info!("Starting ST Language Server");

    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();

    let (service, socket) = LspService::new(StLanguageServer::new);
    Server::new(stdin, stdout, socket).serve(service).await;
}
