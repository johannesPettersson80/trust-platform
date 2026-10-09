//! Runtime-to-runtime mesh communication over Zenoh.

#![allow(missing_docs)]

mod catalogue;
mod hmi_operations;
mod hmi_reads;
mod mapping;
mod models;
mod security;
mod startup_publish;
mod version;

pub use models::{
    MeshLivelinessEvent, MeshLivelinessSnapshot, MeshQosProfile, MeshReadiness, MeshService,
    MeshTopologyEvidence,
};
pub use startup_publish::{start_mesh, start_mesh_with_tls};
pub use version::{validate_zenoh_version_policy, ZENOHD_BASELINE_VERSION, ZENOH_BASELINE_VERSION};

#[cfg(test)]
mod tests {
    include!("tests.rs");
}

#[cfg(test)]
mod security_tests;

#[cfg(test)]
mod hmi_remote_tests;
