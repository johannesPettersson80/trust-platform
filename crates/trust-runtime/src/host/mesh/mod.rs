//! Runtime-to-runtime mesh communication over Zenoh.

#![allow(missing_docs)]

mod mapping;
mod models;
mod startup_publish;
mod version;

pub use models::{
    MeshLivelinessEvent, MeshLivelinessSnapshot, MeshQosProfile, MeshReadiness, MeshService,
};
pub use startup_publish::start_mesh;
pub use version::{validate_zenoh_version_policy, ZENOHD_BASELINE_VERSION, ZENOH_BASELINE_VERSION};

#[cfg(test)]
mod tests {
    include!("tests.rs");
}
