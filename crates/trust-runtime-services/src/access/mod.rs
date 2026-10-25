//! Access rights of the control connection (specification 11, §6.9.5 and §6.9.6): roles, the
//! operation registry, the minimum role of each request, the role a request acts with, the
//! credential channel of communication requests, and the pairings that give a client a
//! role-scoped token.

mod auth;
mod credential_channel;
pub mod pairing;
mod policy;
mod registry;
mod role;

pub use auth::{resolve_role, AuthFailure};
pub use credential_channel::{credential_channel_for_client, stamp_credential_channel_param};
pub use pairing::PairingService;
pub use policy::{is_debug_request, required_role_for_control_request};
pub use role::{constant_time_eq, AccessRole};
