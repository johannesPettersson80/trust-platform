//! The mesh peers of a runtime in a search (specification 11, "Sources" and O31): the runtimes
//! that the runtime's mesh session has seen join are asked for their identity with a read-only
//! query, each second until the search ends, and each answer is a truST runtime find.

use std::time::Duration;

use serde_json::json;

use crate::connectors::DiscoveryConfidence;
use crate::mesh::{query_identities, MeshIdentityReply};

use super::super::model::{DeviceKey, Identity, ProtocolFind, RuntimeIdentity, SourceId};
use super::super::runner::SourceSink;
use super::find::now_ms;
use super::RuntimeFacts;

/// How often the mesh peers are asked again.
const MESH_QUERY_PERIOD: Duration = Duration::from_secs(1);
/// The longest that one round of identity queries waits for its answers.
const MESH_QUERY_TIMEOUT: Duration = Duration::from_secs(1);
/// The warning of a mesh find, which names no control endpoint.
const MESH_NO_CONTROL_WARNING: &str =
    "Runtime found through the mesh without a control endpoint; adopt may require manual endpoint entry.";

/// Asks the mesh peers for their identity until the search ends. Without a mesh session there are
/// no peers to ask. An answer that gives the search runtime's own stable runtime ID is left out: a
/// runtime's search never reports the runtime itself (specification 11, "One search, device identity,
/// and presence").
pub(super) fn search(runtime: &RuntimeFacts, sink: &SourceSink) {
    let Some(mesh) = runtime.mesh.as_ref() else {
        return;
    };
    let Some(session) = mesh.session() else {
        return;
    };
    while !sink.ended() {
        let peers = mesh.liveliness_snapshot().peers;
        let timeout = sink.remaining().min(MESH_QUERY_TIMEOUT);
        for (peer, answer) in query_identities(session, &peers, timeout) {
            match answer {
                Ok(reply)
                    if reply.stable_runtime_id.as_deref() == Some(runtime.runtime_id.as_str()) => {}
                Ok(reply) => sink.found(mesh_find(&reply)),
                Err(reason) => sink.warn(format!("{peer}: {reason}")),
            }
        }
        sink.wait(MESH_QUERY_PERIOD);
    }
}

/// The truST runtime find of one identity answer. It is keyed by the runtime ID that the answer
/// gives as `stable_runtime_id`, the one that the mDNS announcement and `status` give, so that the
/// find merges with the mDNS find of the same runtime; an answer without one is keyed by its mesh
/// name. It is named by the name that the answer sent, or by its mesh name when it sends none
/// (specification 11, "Sources").
fn mesh_find(reply: &MeshIdentityReply) -> ProtocolFind {
    let mesh_name = reply.identity.runtime_id.clone();
    let id = reply
        .stable_runtime_id
        .clone()
        .unwrap_or_else(|| mesh_name.clone());
    let name = reply.name.clone().unwrap_or(mesh_name);
    // As a find without an address names its host by its name, a mesh find does the same.
    let params = json!({
        "control_endpoint": "",
        "name": name,
        "host": name,
        "web_port": null,
        "host_group": null,
    });
    let identity = Identity::TrustRuntime(RuntimeIdentity {
        name: name.clone(),
        id: id.clone(),
        control: None,
        web: None,
        host_group: None,
        mesh_port: None,
        version: reply.version.clone(),
        platform: reply.platform.clone(),
    });
    ProtocolFind::new(
        SourceId::TrustRuntime,
        "mesh",
        DiscoveryConfidence::Confirmed,
        identity,
        params,
        now_ms(),
    )
    .with_warnings(vec![MESH_NO_CONTROL_WARNING.to_string()])
    .keyed(format!("mesh:{id}"), vec![DeviceKey::RuntimeId(id)])
}

#[cfg(test)]
#[path = "mesh_peers_tests.rs"]
mod mesh_peers_tests;
