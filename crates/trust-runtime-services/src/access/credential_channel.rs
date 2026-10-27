//! The credential channel of a communication request: whether the secrets in its parameters
//! reached the runtime without leaving the host (specification 11, §6.9.7).

use alloc::string::ToString;
use core::net::SocketAddr;

use serde_json::json;

use crate::protocol::ControlRequest;

/// Records in the parameters of a communication request which channel its secrets came over.
///
/// It applies to `comm.apply`, `comm.test`, `comm.browse_symbols` and
/// `ads.import_symbols.apply`. The value replaces a `credential_channel` the caller supplied;
/// absent parameters become an object, and parameters that are not an object stay as they are.
#[allow(
    clippy::missing_panics_doc,
    reason = "the serde_json::json! conversions of these values cannot fail"
)]
pub fn stamp_credential_channel_param(request: &mut ControlRequest, client: Option<&str>) {
    if !matches!(
        request.r#type.as_str(),
        "comm.apply" | "comm.test" | "comm.browse_symbols" | "ads.import_symbols.apply"
    ) {
        return;
    }
    let channel = credential_channel_for_client(client);
    let params = request.params.get_or_insert_with(|| json!({}));
    if let Some(object) = params.as_object_mut() {
        object.insert("credential_channel".to_string(), json!(channel));
    }
}

/// `trusted_same_host` for a client on the same host, otherwise `untrusted_remote_plain_tcp`.
///
/// A client is on the same host without a transport label, on the Unix socket (`unix`), with the
/// `loopback` label or with a loopback socket address.
#[must_use]
pub fn credential_channel_for_client(client: Option<&str>) -> &'static str {
    let Some(client) = client else {
        return "trusted_same_host";
    };
    if client == "unix" || client == "loopback" {
        return "trusted_same_host";
    }
    if let Ok(addr) = client.parse::<SocketAddr>()
        && addr.ip().is_loopback()
    {
        return "trusted_same_host";
    }
    "untrusted_remote_plain_tcp"
}

#[cfg(test)]
mod contract_tests;
