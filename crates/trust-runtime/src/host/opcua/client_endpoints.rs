//! OPC UA endpoint information from a server's `GetEndpoints` answer: the
//! application and product URIs, every announced endpoint with its security
//! policy and mode, and the server-certificate fingerprint (spec 11, "One
//! search, device identity, and presence").

use super::{OpcUaMessageSecurityMode, OpcUaSecurityPolicy};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientEndpointInfo {
    pub endpoint_url: String,
    pub application_name: Option<String>,
    /// The server's application URI, when it announces one.
    pub application_uri: Option<String>,
    /// The server's product URI, when it announces one.
    pub product_uri: Option<String>,
    /// The announced security policy, when truST knows it.
    pub security_policy: Option<OpcUaSecurityPolicy>,
    /// The announced security-policy URI, as the server wrote it.
    pub security_policy_uri: String,
    /// The announced security mode, when truST knows it.
    pub security_mode: Option<OpcUaMessageSecurityMode>,
    pub anonymous_supported: bool,
    pub username_supported: bool,
    /// Whether the endpoint announces a security policy or mode that truST cannot use.
    pub unsupported: bool,
    /// The SHA-256 fingerprint of the server certificate the endpoint announces, when it has one.
    pub certificate_sha256: Option<String>,
}

#[cfg(feature = "opcua-wire")]
pub(crate) fn endpoint_info_from_wire(
    endpoint: ::opcua::types::EndpointDescription,
) -> OpcUaClientEndpointInfo {
    let security_policy_uri = endpoint.security_policy_uri.as_ref().to_string();
    let security_policy = security_policy_from_uri(security_policy_uri.as_str());
    let security_mode = security_mode_from_wire(endpoint.security_mode);
    let (anonymous_supported, username_supported) =
        endpoint
            .user_identity_tokens
            .as_ref()
            .map_or((false, false), |tokens| {
                let anonymous = tokens.iter().any(|token| {
                    matches!(token.token_type, ::opcua::types::UserTokenType::Anonymous)
                });
                let username = tokens.iter().any(|token| {
                    matches!(token.token_type, ::opcua::types::UserTokenType::UserName)
                });
                (anonymous, username)
            });
    let certificate_sha256 = endpoint_certificate_sha256(endpoint.server_certificate.as_ref());
    let server = endpoint.server;
    OpcUaClientEndpointInfo {
        endpoint_url: endpoint.endpoint_url.to_string(),
        application_name: super::localized_text(server.application_name),
        application_uri: non_empty_ua_string(server.application_uri.as_ref()),
        product_uri: non_empty_ua_string(server.product_uri.as_ref()),
        unsupported: security_policy.is_none() || security_mode.is_none(),
        security_policy,
        security_policy_uri,
        security_mode,
        anonymous_supported,
        username_supported,
        certificate_sha256,
    }
}

/// The SHA-256 fingerprint of an endpoint's server certificate, lowercase hex; `None` without one.
#[cfg(feature = "opcua-wire")]
fn endpoint_certificate_sha256(certificate: &[u8]) -> Option<String> {
    if certificate.is_empty() {
        return None;
    }
    use sha2::Digest;
    Some(hex::encode(sha2::Sha256::digest(certificate)))
}

/// A UA string field as an owned `String`, trimmed; `None` when it is empty.
#[cfg(feature = "opcua-wire")]
fn non_empty_ua_string(value: &str) -> Option<String> {
    let value = value.trim();
    if value.is_empty() {
        None
    } else {
        Some(value.to_string())
    }
}

#[cfg(feature = "opcua-wire")]
fn security_policy_from_uri(uri: &str) -> Option<OpcUaSecurityPolicy> {
    if uri.ends_with("#None") {
        Some(OpcUaSecurityPolicy::None)
    } else if uri.ends_with("#Basic256Sha256") {
        Some(OpcUaSecurityPolicy::Basic256Sha256)
    } else if uri.ends_with("#Aes128_Sha256_RsaOaep") {
        Some(OpcUaSecurityPolicy::Aes128Sha256RsaOaep)
    } else {
        None
    }
}

#[cfg(feature = "opcua-wire")]
fn security_mode_from_wire(
    mode: ::opcua::types::MessageSecurityMode,
) -> Option<OpcUaMessageSecurityMode> {
    match mode {
        ::opcua::types::MessageSecurityMode::None => Some(OpcUaMessageSecurityMode::None),
        ::opcua::types::MessageSecurityMode::Sign => Some(OpcUaMessageSecurityMode::Sign),
        ::opcua::types::MessageSecurityMode::SignAndEncrypt => {
            Some(OpcUaMessageSecurityMode::SignAndEncrypt)
        }
        _ => None,
    }
}
