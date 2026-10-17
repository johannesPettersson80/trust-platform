use alloc::string::String;

use serde::Deserialize;

/// One control request: `{"id", "type", "params", "auth", "request_id"}`. Members the envelope
/// does not name are ignored.
#[derive(Debug, Deserialize)]
pub struct ControlRequest {
    /// Request number, echoed in the response.
    pub id: u64,
    /// Operation name, for example `status`.
    #[serde(rename = "type")]
    pub r#type: String,
    /// Operation parameters.
    pub params: Option<serde_json::Value>,
    /// Control token or pairing token.
    pub auth: Option<String>,
    /// Caller's correlation id for the audit record; `correlation_id` is accepted as an alias.
    #[serde(default, alias = "correlation_id")]
    pub request_id: Option<String>,
}
