//! `trust-lsp hmi-agent`: the HMI authoring tools of one VS Code window, served to an agent over MCP
//! (specification 43, §3.1).
//!
//! The agent starts this server with the private connection file that the extension's **HMI: Connect
//! Agent Tools** command writes. The server reads one JSON-RPC message per line on standard input and
//! writes each answer as one line on standard output, which carries nothing else. It owns every rule
//! of the MCP connection: the message form, the protocol version, the request ids, the limits and
//! cancellation. A tool call goes to the window's loopback (`agent/bridge.ts` in the extension),
//! which checks the capability and runs the VS Code tool.
//!
//! ```text
//! trust-lsp hmi-agent <connection-file>
//! ```
//!
//! Exit status: 0 when standard input ends, 1 when the connection cannot be opened, 2 for a usage
//! error.

mod bridge;
mod connection;
mod lines;
mod protocol;
mod server;
#[cfg(test)]
mod tests;

use std::path::Path;
use std::time::Duration;

use connection::Connection;
use server::Server;

const USAGE: &str = "usage: trust-lsp hmi-agent <connection-file>

Serves the HMI authoring tools of one VS Code window to an agent over MCP, on standard input and
standard output. The connection file is the one the HMI: Connect Agent Tools command names.

Exit status: 0 when standard input ends, 1 when the connection cannot be opened, 2 usage error.
";

/// What the agent's client shows when the connection cannot be opened (standard error).
const NOT_CONNECTED: &str =
    "Unable to open the private HMI authoring connection. Connect from VS Code first.";

/// The limits of the MCP connection (specification 43, §3.1).
#[derive(Debug, Clone, Copy)]
pub(crate) struct Limits {
    /// The longest message line, in bytes before its line end.
    pub(crate) line_bytes: usize,
    /// The most tool calls in progress at once.
    pub(crate) active_calls: usize,
    /// How long a tool call runs before the server cancels it at the loopback.
    pub(crate) call_time: Duration,
    /// How long one exchange with the loopback may take before it fails.
    pub(crate) bridge_time: Duration,
    /// The largest loopback answer, in raw bytes.
    pub(crate) answer_bytes: usize,
}

impl Limits {
    /// The product's limits: 8 MiB lines, eight calls, 120 seconds per call, 125 seconds per
    /// exchange and 64 MiB answers.
    pub(crate) const PRODUCT: Self = Self {
        line_bytes: 8 * 1024 * 1024,
        active_calls: 8,
        call_time: Duration::from_secs(120),
        bridge_time: Duration::from_secs(125),
        answer_bytes: 64 * 1024 * 1024,
    };
}

/// Runs `trust-lsp hmi-agent` with the arguments after `hmi-agent`; returns the exit status.
pub(crate) fn run(args: &[String]) -> i32 {
    let [file] = args else {
        eprint!("{USAGE}");
        return 2;
    };
    if file.starts_with('-') {
        eprint!("{USAGE}");
        return 2;
    }
    let Ok(connection) = Connection::read(Path::new(file)) else {
        eprintln!("{NOT_CONNECTED}");
        return 1;
    };
    match Server::start(connection, Limits::PRODUCT, Box::new(std::io::stdout())) {
        Ok(server) => {
            server.serve(&mut std::io::stdin().lock());
            0
        }
        Err(_) => {
            eprintln!("{NOT_CONNECTED}");
            1
        }
    }
}
