//! The Devices & Connections panel engine (specification 25 §5.15). It gathers the chosen
//! project, the runtimes it reaches and their discovery results, merges them into one topology,
//! and answers the whole panel view. The extension only draws what this crate answers.

#![allow(clippy::module_name_repetitions)]

pub mod gather;
pub mod request;
pub mod session;
pub mod targets;
pub mod topology;

#[cfg(test)]
mod fake_runtime;
#[cfg(test)]
mod test_project;
