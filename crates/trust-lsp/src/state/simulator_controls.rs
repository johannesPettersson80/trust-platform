//! The simulators that launches start in this session: the endpoint and token that
//! `trust-lsp.simulatorControl` named for each workspace folder (specification 25 §6.8). The
//! language server generated each token itself, so runtime-assisted inline values reach the
//! simulator of a document's workspace folder without a secret crossing the protocol
//! (specification 14, runtime-assisted inline values). Beside them, the runtimes that debug
//! sessions attached to ([`attached_runtimes`]).

use parking_lot::RwLock;
use std::path::{Path, PathBuf};

#[path = "attached_runtimes.rs"]
pub(crate) mod attached_runtimes;

/// The control endpoint and token of a launch's simulator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SimulatorControl {
    pub(crate) endpoint: String,
    pub(crate) auth_token: String,
}

/// The simulator control named for each workspace folder, and for a launch without one, and the
/// runtimes that debug sessions attached to.
#[derive(Default)]
pub(crate) struct SimulatorControls {
    /// Each folder in the form this platform compares ([`compared`]), with its control.
    named: RwLock<Vec<(Option<PathBuf>, SimulatorControl)>>,
    attached: attached_runtimes::AttachedRuntimes,
}

impl SimulatorControls {
    /// The runtimes that debug sessions attached to in this session (specification 25 §6.8).
    pub(crate) fn attached(&self) -> &attached_runtimes::AttachedRuntimes {
        &self.attached
    }

    /// Names a control for `workspace` with `name` and keeps it in place of an earlier one, under
    /// one lock, so that two folders asked for at once never take one endpoint. `name` is given
    /// whether an endpoint is named for a folder other than `workspace`, which a new simulator of
    /// `workspace` passes over.
    pub(crate) fn name_and_record(
        &self,
        workspace: Option<PathBuf>,
        name: impl FnOnce(&dyn Fn(&str) -> bool) -> SimulatorControl,
    ) -> SimulatorControl {
        let key = workspace.as_deref().map(compared);
        let mut named = self.named.write();
        let control = name(&|endpoint: &str| {
            named
                .iter()
                .any(|(folder, control)| *folder != key && control.endpoint == endpoint)
        });
        named.retain(|(folder, _)| *folder != key);
        named.push((key, control.clone()));
        control
    }

    /// The control named for the workspace folder that holds `document`, the innermost when
    /// folders nest, else the one named without a folder.
    pub(crate) fn for_document(&self, document: &Path) -> Option<SimulatorControl> {
        let document = compared(document);
        let named = self.named.read();
        let in_folder = named
            .iter()
            .filter_map(|(folder, control)| Some((folder.as_ref()?, control)))
            .filter(|(folder, _)| document.starts_with(folder))
            .max_by_key(|(folder, _)| folder.components().count())
            .map(|(_, control)| control.clone());
        in_folder.or_else(|| {
            named
                .iter()
                .find(|(folder, _)| folder.is_none())
                .map(|(_, control)| control.clone())
        })
    }
}

/// `path` in the form this platform compares: Windows' file systems ignore case, so a folder and a
/// document's path there match in any case, as `C:\Work` and `c:\work\main.st` do.
fn compared(path: &Path) -> PathBuf {
    if cfg!(windows) {
        PathBuf::from(path.to_string_lossy().to_lowercase())
    } else {
        path.to_path_buf()
    }
}

#[cfg(test)]
#[path = "simulator_controls_tests.rs"]
mod tests;
