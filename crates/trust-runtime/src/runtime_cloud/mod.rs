//! Runtime cloud contracts and UI state projection boundaries.
//!
//! Ownership map (A1.1):
//! - Cloud plane (`runtime_cloud::*`): contract types, reason codes, action routing, and UI
//!   projection rules. No transport/socket dependencies.
//! - Transport plane (`web`, `discovery`, `mesh`): network I/O and request handling only.
//! - Realtime plane (`runtime::mesh`, scheduler/task execution): deterministic task/runtime path.
//! - UI projection boundary (`runtime_cloud::projection`): canonical state mapping from
//!   transport observations into UI contracts.
//!
//! Boundary rule:
//! - transport modules depend on `runtime_cloud`, never the reverse.
//! - realtime execution path is not called directly from runtime-cloud projection/contracts.

pub(crate) mod config_policy;
pub mod contracts;
pub(crate) mod control_proxy_policy;
pub mod ha;
pub(crate) mod io_proxy_policy;
pub mod keyspace;
pub(crate) mod link_policy;
pub(crate) mod profile_policy;
pub mod projection;
pub(crate) mod rollout_policy;
pub mod routing;
