//! OPC UA (specification 11, "comm.discover" and "Sources"): the endpoints that the server at a
//! known address announces in its GetEndpoints answer, each one a candidate. `discover` asks one
//! server for `comm.discover` or for a search, and `search` asks each known address of a search.
//! The GetEndpoints call keeps the timeouts of the OPC UA client, bounded by the search deadline.

use std::time::Duration;

use serde_json::{json, Value};

use super::super::model::{Identity, OpcuaIdentity, ProtocolFind, SourceId};
use super::super::runner::SourceSink;
use super::find::{address_key, find, text, FindSink};
use super::report::{sanitize_id, CandidateSink, Deadline, DiscoverCandidate};
use super::scope::{authority_host_and_port, DiscoverScope};
use super::SourceContext;

const OPCUA_PORT: u16 = 4840;
/// How long one GetEndpoints call may take, even when the search deadline is farther away.
const OPCUA_DISCOVERY_TIMEOUT: Duration = Duration::from_secs(10);
/// The label of an endpoint whose server has no application name.
const UNNAMED_OPCUA_SERVER: &str = "OPC UA server";
/// The warning about an endpoint whose security policy or mode truST cannot use.
const UNSUPPORTED_ENDPOINT_WARNING: &str =
    "The endpoint announces a security policy or mode that truST cannot use; it is listed, not usable.";

/// Asks the server at the known address of `scope` for its endpoints.
pub(crate) fn discover(
    scope: &DiscoverScope,
    deadline: Deadline,
    sink: &mut dyn CandidateSink,
) -> Result<(), String> {
    if deadline.passed() {
        return Ok(());
    }
    let endpoint_url = opcua_endpoint_url(scope)?;
    match crate::opcua::discover_opcua_client_endpoints(
        endpoint_url.as_str(),
        deadline.bound(OPCUA_DISCOVERY_TIMEOUT),
    ) {
        Ok(endpoints) => {
            if endpoints.is_empty() {
                sink.warn(format!(
                    "No OPC UA endpoints were advertised by {endpoint_url}."
                ));
            }
            for endpoint in &endpoints {
                sink.found(endpoint_candidate(endpoint));
            }
        }
        Err(error) => sink.warn(format!("OPC UA discovery did not complete: {error}")),
    }
    Ok(())
}

/// One endpoint the server announced, as a candidate. Every endpoint is listed, including those
/// whose security policy or mode truST cannot use; those carry `unsupported` and a warning
/// (specification 11, "comm.discover").
pub(super) fn endpoint_candidate(
    endpoint: &crate::opcua::OpcUaClientEndpointInfo,
) -> DiscoverCandidate {
    let policy: Option<&str> = endpoint
        .security_policy
        .map(|policy| policy.as_config_value());
    let mode: Option<&str> = endpoint.security_mode.map(|mode| mode.as_config_value());
    let label = endpoint
        .application_name
        .clone()
        .unwrap_or_else(|| UNNAMED_OPCUA_SERVER.to_string());
    let warnings = if endpoint.unsupported {
        vec![UNSUPPORTED_ENDPOINT_WARNING.to_string()]
    } else {
        Vec::new()
    };
    DiscoverCandidate {
        id: format!(
            "opcua:{}:{}:{}",
            sanitize_id(endpoint.endpoint_url.as_str()),
            policy.unwrap_or(endpoint.security_policy_uri.as_str()),
            mode.unwrap_or("unsupported")
        ),
        label,
        source: "opcua_get_endpoints",
        confidence: "confirmed",
        params: json!({
            "endpoint_url": endpoint.endpoint_url,
            "security_policy": policy,
            "security_policy_uri": endpoint.security_policy_uri,
            "security_mode": mode,
            "auth": if endpoint.anonymous_supported { "anonymous" } else { "username" },
            "anonymous_supported": endpoint.anonymous_supported,
            "username_supported": endpoint.username_supported,
            "unsupported": endpoint.unsupported,
            "application_uri": endpoint.application_uri,
            "product_uri": endpoint.product_uri,
            "certificate_sha256": endpoint.certificate_sha256,
            "trust_server_certificate": false,
        }),
        warnings,
    }
}

fn opcua_endpoint_url(scope: &DiscoverScope) -> Result<String, String> {
    let raw = scope
        .host
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| "OPC UA client discovery needs scope.host".to_string())?;
    let endpoint = if let Some(endpoint) = raw.strip_prefix("opc.tcp://") {
        endpoint
    } else {
        if raw.contains("://") {
            return Err("OPC UA endpoint must use the opc.tcp scheme".to_string());
        }
        raw
    };
    if endpoint.contains(['?', '#']) {
        return Err("OPC UA endpoint must not contain a query or fragment".to_string());
    }
    let (authority, path) = endpoint
        .split_once('/')
        .map_or((endpoint, ""), |(authority, path)| (authority, path));
    let (host, port) = authority_host_and_port(authority, "OPC UA")?;
    let authority = match (port, path.is_empty()) {
        (Some(port), _) => format!("{host}:{port}"),
        (None, true) => format!("{host}:{OPCUA_PORT}"),
        (None, false) => host,
    };
    if path.is_empty() {
        Ok(format!("opc.tcp://{authority}"))
    } else {
        Ok(format!("opc.tcp://{authority}/{path}"))
    }
}

/// OPC UA in a search: the endpoints of each known address, one after another.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        let scope = DiscoverScope {
            host: Some(host.clone()),
            ..DiscoverScope::default()
        };
        let mut found = FindSink::new(sink, Some(host.as_str()), |candidate| {
            opcua_find(candidate, host)
        });
        if let Err(error) = discover(&scope, found.deadline(), &mut found) {
            sink.warn(format!("{host}: {error}"));
        }
    }
}

/// One endpoint that the server at `host` announced; the device is the probed host, because an
/// endpoint URL may name the server by another name.
fn opcua_find(candidate: DiscoverCandidate, host: &str) -> ProtocolFind {
    let endpoint_url = text(&candidate.params, "endpoint_url").unwrap_or_default();
    let security_policy = text(&candidate.params, "security_policy");
    let security_mode = text(&candidate.params, "security_mode");
    let application_name = Some(candidate.label.clone())
        .filter(|label| !label.is_empty() && label != UNNAMED_OPCUA_SERVER);
    let key = format!(
        "{endpoint_url} {} {}",
        security_policy.as_deref().unwrap_or_default(),
        security_mode.as_deref().unwrap_or_default()
    );
    let keys = address_key(host).into_iter().collect();
    let identity = Identity::Opcua(OpcuaIdentity {
        endpoint_url,
        security_policy,
        security_mode,
        application_name,
        application_uri: text(&candidate.params, "application_uri"),
        product_uri: text(&candidate.params, "product_uri"),
        unsupported: candidate
            .params
            .get("unsupported")
            .and_then(Value::as_bool)
            .unwrap_or(false),
        certificate_sha256: text(&candidate.params, "certificate_sha256"),
    });
    find(SourceId::Opcua, candidate, identity).keyed(key, keys)
}

#[cfg(test)]
#[path = "opcua_tests.rs"]
mod tests;
