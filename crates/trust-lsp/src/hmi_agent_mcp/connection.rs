//! The private connection file (specification 43, §3.1): `{ "url", "capability", "workspaceId" }`,
//! written by the extension's HMI: Connect Agent Tools command for one workspace of one window.

use std::fmt;
use std::fs::Metadata;
use std::net::{Ipv4Addr, SocketAddr};
use std::path::Path;

use serde::Deserialize;
use tower_lsp::lsp_types::Url;

/// A checked connection: where the window's loopback listens, its capability and its workspace.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Connection {
    /// The loopback's address: 127.0.0.1 and the port of the connection's URL.
    pub(crate) address: SocketAddr,
    /// The session capability, the bearer token of every request to the loopback.
    pub(crate) capability: String,
    /// The workspace the connection serves; every tool call is made for it.
    pub(crate) workspace_id: String,
}

/// The capability never enters output, not even a debug print.
impl fmt::Debug for Connection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connection")
            .field("address", &self.address)
            .field("workspace_id", &self.workspace_id)
            .finish_non_exhaustive()
    }
}

/// Why a connection file is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Refusal {
    /// The file cannot be read.
    Unreadable,
    /// Other users than its owner may access the file (Unix).
    NotPrivate,
    /// The file is not a connection object.
    Malformed,
    /// The URL is not `http://127.0.0.1:<port>/mcp` without credentials, query or fragment.
    NotLoopback,
    /// The capability is not 64 lowercase hexadecimal digits.
    Capability,
    /// The workspace identity is empty.
    Workspace,
}

/// The connection file's fields.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ConnectionFile {
    url: String,
    capability: String,
    workspace_id: String,
}

impl Connection {
    /// Reads and checks the connection file at `path`.
    pub(crate) fn read(path: &Path) -> Result<Self, Refusal> {
        let metadata = std::fs::metadata(path).map_err(|_| Refusal::Unreadable)?;
        if !owner_only(&metadata) {
            return Err(Refusal::NotPrivate);
        }
        let text = std::fs::read_to_string(path).map_err(|_| Refusal::Unreadable)?;
        Self::parse(&text)
    }

    /// Checks the text of a connection file.
    pub(crate) fn parse(text: &str) -> Result<Self, Refusal> {
        let file: ConnectionFile = serde_json::from_str(text).map_err(|_| Refusal::Malformed)?;
        let address = loopback_address(&file.url).ok_or(Refusal::NotLoopback)?;
        if !is_capability(&file.capability) {
            return Err(Refusal::Capability);
        }
        if file.workspace_id.is_empty() {
            return Err(Refusal::Workspace);
        }
        Ok(Self {
            address,
            capability: file.capability,
            workspace_id: file.workspace_id,
        })
    }
}

/// The address of a loopback URL `http://127.0.0.1:<port>/mcp` without credentials, query or
/// fragment, read as a browser reads it; any other URL has none.
fn loopback_address(text: &str) -> Option<SocketAddr> {
    let url = Url::parse(text).ok()?;
    let plain = url.scheme() == "http"
        && url.host_str() == Some("127.0.0.1")
        && url.path() == "/mcp"
        && url.username().is_empty()
        && url.password().is_none()
        && url.query().is_none()
        && url.fragment().is_none();
    let port = url.port_or_known_default()?;
    plain.then_some(SocketAddr::from((Ipv4Addr::LOCALHOST, port)))
}

/// Whether `text` is a capability: 64 lowercase hexadecimal digits.
fn is_capability(text: &str) -> bool {
    text.len() == 64
        && text
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

/// Whether only the file's owner may access it: no group or other permission bits.
#[cfg(unix)]
fn owner_only(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o077 == 0
}

/// Windows keeps the file in the user's private extension storage; it has no mode bits to check.
#[cfg(not(unix))]
fn owner_only(_metadata: &Metadata) -> bool {
    true
}
