//! OPC UA servers that a local discovery server lists (specification 11, "Sources"): a search
//! asks this computer's own discovery server and the discovery server of each known address for
//! `FindServers`, and then asks every discovery URL that a listed server names for its endpoints.
//! Only an endpoint answer is a device; a discovery server that does not answer is skipped
//! silently for `localhost`, and is a warning for a known address.

use crate::opcua::OpcUaFoundServer;

use super::super::runner::SourceSink;
use super::opcua::{opcua_endpoint_finds, opcua_probe_timeout};
use super::scope::authority_host_and_port;
use super::SourceContext;

/// This computer's own local discovery server, which a search always asks
/// (specification 11, "Sources").
pub(super) const LOCAL_DISCOVERY_SERVER: &str = "opc.tcp://localhost:4840";
/// The port of the discovery server of a known address that names none.
const OPCUA_PORT: u16 = 4840;
/// How a find through a discovery server is named (specification 11, "Sources").
const LDS_SOURCE: &str = "lds";

/// OPC UA servers in a search: the servers that the local discovery servers list.
pub(super) fn search(context: &SourceContext, sink: &SourceSink) {
    if sink.ended() {
        return;
    }
    ask_discovery_server(LOCAL_DISCOVERY_SERVER, true, sink);
    for host in &context.hosts {
        if sink.ended() {
            return;
        }
        match discovery_url(host) {
            Ok(url) => ask_discovery_server(url.as_str(), false, sink),
            Err(error) => sink.warn(format!("{host}: {error}")),
        }
    }
}

/// The discovery-server URL of a known address: `opc.tcp://<host>:<port>`, port 4840 unless the
/// address names one (specification 11, "Sources").
pub(super) fn discovery_url(host: &str) -> Result<String, String> {
    let (host, port) = authority_host_and_port(host, "OPC UA")?;
    Ok(format!("opc.tcp://{host}:{}", port.unwrap_or(OPCUA_PORT)))
}

/// Asks `discovery_url` for the servers it lists, and then asks every discovery URL those servers
/// name for their endpoints (specification 11, "Sources").
fn ask_discovery_server(discovery_url: &str, local: bool, sink: &SourceSink) {
    let answer =
        crate::opcua::discover_opcua_discovery_servers(discovery_url, opcua_probe_timeout(sink));
    let servers = match answer {
        Ok(servers) => servers,
        // A computer often runs no local discovery server; its silence is not a warning.
        Err(_) if local => return,
        Err(error) => {
            sink.warn(format!("{discovery_url}: {error}"));
            return;
        }
    };
    let (urls, warnings) = discovery_urls(&servers);
    for warning in warnings {
        sink.warn(warning);
    }
    for url in urls {
        if sink.ended() {
            return;
        }
        ask_endpoints(url.as_str(), sink);
    }
}

/// What a `FindServers` answer names: the discovery URLs to ask with GetEndpoints, in order, and a
/// warning for every server that names none (specification 11, "Sources").
pub(super) fn discovery_urls(servers: &[OpcUaFoundServer]) -> (Vec<String>, Vec<String>) {
    let mut urls = Vec::new();
    let mut warnings = Vec::new();
    for server in servers {
        if server.discovery_urls.is_empty() {
            warnings.push(format!(
                "The discovery server listed {} without a discovery URL.",
                server_name(server)
            ));
            continue;
        }
        urls.extend(server.discovery_urls.iter().cloned());
    }
    (urls, warnings)
}

/// The name of one server of a `FindServers` answer, or that it announced none.
fn server_name(server: &OpcUaFoundServer) -> &str {
    server
        .application_name
        .as_deref()
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .unwrap_or("an unnamed OPC UA server")
}

/// Asks the server at `endpoint_url` for its endpoints and reports each one as an `lds` find.
fn ask_endpoints(endpoint_url: &str, sink: &SourceSink) {
    let answer =
        crate::opcua::discover_opcua_client_endpoints(endpoint_url, opcua_probe_timeout(sink));
    match opcua_endpoint_finds(endpoint_url, answer, LDS_SOURCE) {
        Ok(finds) => {
            for find in finds {
                sink.found(find);
            }
        }
        Err(warning) => sink.warn(warning),
    }
}

#[cfg(test)]
#[path = "opcua_lds_tests.rs"]
mod tests;
