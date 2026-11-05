//! An MCP server on standard input and standard output (specification 43, §3.1). It owns every
//! rule of the MCP connection: the message form, the protocol version, the request ids,
//! cancellation, and how the [`Limits`] it is given apply. What the tools are and how a call runs
//! belong to a [`ToolProvider`]:
//! `trust-lsp hmi-agent` provides the HMI authoring tools of one VS Code window
//! ([`crate::hmi_agent_mcp`]), and another provider plugs into the same server.

mod lines;
mod protocol;
mod server;
#[cfg(test)]
mod tests;

use std::time::Duration;

use serde_json::{Map, Value};

pub use server::{Server, StartError};

/// The limits of an MCP connection (specification 43, §3.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The longest message line, in bytes before its line end.
    pub line_bytes: usize,
    /// The most tool calls in progress at once; one more is refused (-32600). A call counts until
    /// its provider's [`ToolProvider::call`] returns, also after its failure answer. A provider
    /// whose calls must not overlap serializes them itself, for example behind a lock in `call`,
    /// and allows more than one call, so that calls wait instead of being refused.
    pub active_calls: usize,
    /// How long a tool call runs before the server cancels it at its provider.
    pub call_time: Duration,
    /// How long a tool call may take before the server answers it with the failure answer.
    pub answer_time: Duration,
}

/// What a server says of itself: in the answer to `initialize`, and in the failure answer (-32000)
/// of a call that its provider could not answer or that the server could not run, and of a line
/// over the line limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ServerInfo {
    /// `serverInfo.name`.
    pub name: &'static str,
    /// `serverInfo.version`.
    pub version: &'static str,
    /// The instructions for the agent.
    pub instructions: &'static str,
    /// The message of the failure answer.
    pub unavailable: &'static str,
}

/// A call that its provider could not answer; the server answers it with the failure answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Unavailable;

/// The tools that an MCP server serves, and how their calls run.
pub trait ToolProvider: Send + Sync + 'static {
    /// What the server says of itself.
    fn info(&self) -> ServerInfo;

    /// The tools, as `tools/list` answers them.
    fn tools(&self) -> &[Value];

    /// The checks of a `tools/call` with the parameters `params`: the name of the tool and the
    /// arguments it runs with, or the reason that the server answers with -32602.
    fn check(
        &self,
        params: &Map<String, Value>,
    ) -> Result<(String, Map<String, Value>), &'static str>;

    /// Runs the call that the server knows by `id`: the tool's result, or [`Unavailable`]. The
    /// server runs it on a thread of its own, and it may wait until the tool answers; a call that
    /// panics is answered with the failure answer. Until it returns, it holds one of the
    /// [`Limits::active_calls`].
    fn call(
        &self,
        id: &str,
        name: &str,
        arguments: Map<String, Value>,
    ) -> Result<Value, Unavailable>;

    /// Asks the call that the server knows by `id` to end; the call then answers through
    /// [`ToolProvider::call`]. The server cancels a call at most once, on a thread of its own, when
    /// the agent cancels it or its [`Limits::call_time`] ends. The cancellation may come before
    /// `call` began to run, or after it returned.
    fn cancel(&self, id: &str);
}
