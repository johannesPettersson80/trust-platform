//! The runtimes that debug sessions attached to in this session: the endpoint that the extension
//! named for each workspace folder (`trust-lsp.attachedRuntime`, specification 25 §6.8). The
//! editor names no token: the language server finds the runtime of the folder's project as
//! `trust-runtime ctl --project` does and sends its token only when that runtime's endpoint is the
//! attached one (specification 14, runtime-assisted inline values).

use parking_lot::RwLock;
use std::path::{Path, PathBuf};

use trust_runtime::control::ControlEndpoint;

use super::compared;

/// The endpoint that a debug session of a workspace folder attached to.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct AttachedRuntime {
    /// The workspace folder as the extension named it, `None` for a session without one.
    pub(crate) folder: Option<PathBuf>,
    /// The control endpoint the session attached to.
    pub(crate) endpoint: String,
}

impl AttachedRuntime {
    /// The token to send to the attached runtime: that of the folder's own runtime, found as
    /// `trust-runtime ctl --project` finds it (specification 11 §6.8.0a: the endpoint file, the
    /// token it publishes, or the control token of the project's `runtime.toml`), when that
    /// runtime's endpoint is the attached one; `None` otherwise, and without a folder.
    pub(crate) fn token(&self) -> Option<String> {
        let folder = self.folder.as_deref()?;
        let target =
            trust_runtime::control::endpoint::resolve_control_target(Some(folder), None, None)
                .ok()?;
        if same_endpoint(&target.endpoint_text, &self.endpoint) {
            target.auth_token
        } else {
            None
        }
    }
}

/// Whether the endpoint texts `left` and `right` name one endpoint: the same TCP address, or the
/// same socket path; texts that do not parse are compared as they are, trimmed.
fn same_endpoint(left: &str, right: &str) -> bool {
    match (ControlEndpoint::parse(left), ControlEndpoint::parse(right)) {
        (Ok(ControlEndpoint::Tcp(left)), Ok(ControlEndpoint::Tcp(right))) => left == right,
        #[cfg(unix)]
        (Ok(ControlEndpoint::Unix(left)), Ok(ControlEndpoint::Unix(right))) => left == right,
        // A TCP endpoint and a socket; on Windows every control endpoint is a TCP one.
        #[cfg(unix)]
        (Ok(_), Ok(_)) => false,
        _ => left.trim() == right.trim(),
    }
}

/// The runtime attached for each workspace folder, and for a session without one.
#[derive(Default)]
pub(crate) struct AttachedRuntimes {
    /// Each folder in the form this platform compares ([`compared`]), with its runtime.
    attached: RwLock<Vec<(Option<PathBuf>, AttachedRuntime)>>,
}

impl AttachedRuntimes {
    /// Keeps that a debug session of `folder` attached to `endpoint` in place of an earlier one, or,
    /// with `None`, that the folder's session ended.
    pub(crate) fn record(&self, folder: Option<PathBuf>, endpoint: Option<String>) {
        let key = folder.as_deref().map(compared);
        let mut attached = self.attached.write();
        attached.retain(|(existing, _)| *existing != key);
        if let Some(endpoint) = endpoint {
            attached.push((key, AttachedRuntime { folder, endpoint }));
        }
    }

    /// The runtime attached for the workspace folder that holds `document`, the innermost when
    /// folders nest, else the one attached without a folder.
    pub(crate) fn for_document(&self, document: &Path) -> Option<AttachedRuntime> {
        let document = compared(document);
        let attached = self.attached.read();
        let in_folder = attached
            .iter()
            .filter_map(|(folder, runtime)| Some((folder.as_ref()?, runtime)))
            .filter(|(folder, _)| document.starts_with(folder))
            .max_by_key(|(folder, _)| folder.components().count())
            .map(|(_, runtime)| runtime.clone());
        in_folder.or_else(|| {
            attached
                .iter()
                .find(|(folder, _)| folder.is_none())
                .map(|(_, runtime)| runtime.clone())
        })
    }
}

#[cfg(test)]
#[path = "attached_runtimes_tests.rs"]
mod tests;
