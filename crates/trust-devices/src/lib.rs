//! The Devices & Connections panel engine (specification 25 §5.15). It gathers the chosen
//! project, the runtimes it reaches and their discovery results, merges them into one topology,
//! and answers the whole panel view. The extension only draws what this crate answers.

#![allow(clippy::module_name_repetitions)]

/// The panel's action path: one action that a view offered, decoded and performed here.
pub mod actions;
/// The browse panes: opening a browse at the runtime that owns a connection, and adding its
/// selection (specification 25 §5.4 and §11.2).
pub mod browse;
/// The engine the five `trust-lsp.devices*` commands answer through (design §1.5): the sessions
/// behind a lock, the background threads, and the pending results of slow actions.
pub mod engine;
pub mod gather;
mod inputs;
/// The panes of the panel: the add dialog, the forms and the USB tray.
pub mod panes;
pub mod request;
pub mod session;
pub mod targets;
pub mod texts;
pub mod topology;
pub mod view;

#[cfg(test)]
mod fake_runtime;
#[cfg(test)]
mod test_project;
