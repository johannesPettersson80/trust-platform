//! The simulators that launches start in this session: the endpoint and token that
//! `trust-lsp.simulatorControl` named for each workspace folder (specification 25 §6.8). The
//! language server generated each token itself, so runtime-assisted inline values reach the
//! simulator of a document's workspace folder without a secret crossing the protocol
//! (specification 14, runtime-assisted inline values).

use parking_lot::RwLock;
use std::path::{Path, PathBuf};

/// The control endpoint and token of a launch's simulator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct SimulatorControl {
    pub(crate) endpoint: String,
    pub(crate) auth_token: String,
}

/// The simulator control named for each workspace folder, and for a launch without one.
#[derive(Default)]
pub(crate) struct SimulatorControls {
    named: RwLock<Vec<(Option<PathBuf>, SimulatorControl)>>,
}

impl SimulatorControls {
    /// Keeps `control` as the one named for `workspace`, in place of an earlier one.
    pub(crate) fn record(&self, workspace: Option<PathBuf>, control: SimulatorControl) {
        let mut named = self.named.write();
        named.retain(|(folder, _)| folder != &workspace);
        named.push((workspace, control));
    }

    /// The control named for the workspace folder that holds `document`, the innermost when
    /// folders nest, else the one named without a folder.
    pub(crate) fn for_document(&self, document: &Path) -> Option<SimulatorControl> {
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

#[cfg(test)]
#[path = "simulator_controls_tests.rs"]
mod tests;
