//! The live discovery and mesh health of one `comm.capabilities` answer (specification 11,
//! "Communication schema, capabilities, and Test").

use super::contract::{
    CommCapabilitiesResponse, CommId, DiscoveryHealth, MeshHealth, MeshPeerHealth,
};
use super::ControlState;
use crate::control::fleet_handlers::mesh_peer_matches_target;
use crate::mesh::MeshTopologyEvidence;

/// Fills the live discovery and mesh health into the entries that carry it (specification 11,
/// "Communication schema, capabilities, and Test"). Every other entry keeps its version-4 shape.
pub(super) fn apply_live_health(response: &mut CommCapabilitiesResponse, state: &ControlState) {
    for entry in &mut response.capabilities {
        match entry.id {
            CommId::Discovery => entry.discovery = Some(discovery_health(state)),
            CommId::Mesh => entry.mesh = Some(mesh_health(state)),
            _ => {}
        }
    }
}

/// The live discovery health of one runtime (specification 11, "Communication schema,
/// capabilities, and Test"). It sees the announcements of the other runtimes, never its own.
fn discovery_health(state: &ControlState) -> DiscoveryHealth {
    let settings = state.settings.lock().ok().map(|guard| guard.clone());
    DiscoveryHealth {
        enabled: settings
            .as_ref()
            .is_some_and(|settings| settings.discovery.enabled),
        announcing: settings
            .as_ref()
            .is_some_and(|settings| settings.discovery.enabled && settings.discovery.advertise),
        visible_runtimes: state
            .discovery
            .snapshot()
            .iter()
            .filter(|entry| entry.id != state.runtime_id)
            .count(),
    }
}

/// The live mesh health of one runtime: every configured peer, in configuration order
/// (specification 11, "Communication schema, capabilities, and Test").
fn mesh_health(state: &ControlState) -> MeshHealth {
    let targets = state
        .settings
        .lock()
        .ok()
        .map(|guard| guard.mesh.connect.clone())
        .unwrap_or_default();
    let evidence = state
        .mesh_topology
        .lock()
        .ok()
        .and_then(|guard| guard.clone());
    MeshHealth {
        peers: targets
            .iter()
            .map(|target| mesh_peer_health(target.as_str(), evidence.as_ref()))
            .collect(),
    }
}

/// One configured mesh peer's state (specification 11, "Communication schema, capabilities, and
/// Test"). It is connected while the mesh session is ready and its liveliness reports a peer that
/// `target` names, and it was last heard from at the newest liveliness event that names it. The
/// target matching is the one shared with the fleet mesh link projection
/// (`mesh_peer_matches_target`).
fn mesh_peer_health(target: &str, evidence: Option<&MeshTopologyEvidence>) -> MeshPeerHealth {
    let snapshot = evidence.map(MeshTopologyEvidence::liveliness_snapshot);
    let ready = evidence.is_some_and(MeshTopologyEvidence::is_ready);
    MeshPeerHealth {
        peer: target.to_string(),
        connected: ready
            && snapshot.as_ref().is_some_and(|snapshot| {
                snapshot
                    .peers
                    .iter()
                    .any(|peer| mesh_peer_matches_target(target, peer.as_str()))
            }),
        last_seen_ms: snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .history
                .iter()
                .filter(|event| mesh_peer_matches_target(target, event.runtime_id.as_str()))
                .map(|event| event.timestamp_ns / 1_000_000)
                .max()
        }),
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use smol_str::SmolStr;

    use super::super::handle_comm_capabilities;
    use crate::discovery::DiscoveryEntry;
    use crate::mesh::MeshTopologyEvidence;

    fn discovery_entry(name: &str) -> DiscoveryEntry {
        DiscoveryEntry {
            id: SmolStr::new(name),
            name: SmolStr::new(name),
            addresses: Vec::new(),
            web_port: None,
            web_tls: false,
            mesh_port: None,
            control: None,
            host_group: None,
            last_seen_ns: 1,
        }
    }

    // Specification 11, "Communication schema, capabilities, and Test": the `discovery` entry
    // reports whether discovery is enabled and announcing and how many other runtimes it sees, and
    // the `mesh` entry reports every configured peer, in configuration order, with whether the
    // mesh reports it connected and when it was last heard from. It would catch a missing health
    // key, the runtime's own announcement counted among the visible runtimes, a connected flag that
    // ignores the mesh's liveliness, a fabricated last-seen timestamp for a peer the mesh never
    // heard from, or peers out of order.
    #[test]
    fn capabilities_report_discovery_and_mesh_peer_health() {
        let state = crate::control::tests::hmi_test_state("PROGRAM Main\nEND_PROGRAM\n");
        {
            let mut settings = state.settings.lock().expect("settings lock");
            settings.discovery.enabled = true;
            settings.discovery.advertise = true;
            settings.mesh.connect = vec![
                SmolStr::new("tcp/peer-a:7447"),
                SmolStr::new("tcp/peer-b:7447"),
            ];
        }
        // The snapshot holds this runtime's own announcement and one other runtime; only the other
        // runtime is visible.
        state.discovery.replace_entries(vec![
            discovery_entry(state.runtime_id.as_str()),
            discovery_entry("runtime-b"),
        ]);
        *state.mesh_topology.lock().expect("mesh topology lock") = Some(
            MeshTopologyEvidence::for_test(true, &["peer-a"], 1_700_000_000_000_000_000),
        );

        let response = handle_comm_capabilities(7, &state);
        assert!(response.ok, "capabilities failed: {:?}", response.error);
        let result = response.result.expect("capabilities result");
        let capabilities = result["capabilities"]
            .as_array()
            .expect("capabilities array");
        let entry = |id: &str| {
            capabilities
                .iter()
                .find(|entry| entry["id"] == json!(id))
                .expect("capability entry")
        };
        assert_eq!(entry("discovery")["enabled"], json!(true));
        assert_eq!(entry("discovery")["announcing"], json!(true));
        assert_eq!(entry("discovery")["visible_runtimes"], json!(1));
        assert_eq!(
            entry("mesh")["peers"],
            json!([
                {
                    "peer": "tcp/peer-a:7447",
                    "connected": true,
                    "last_seen_ms": 1_700_000_000_000u64,
                },
                {"peer": "tcp/peer-b:7447", "connected": false, "last_seen_ms": null},
            ])
        );

        // `announcing` follows the advertisement, not discovery being enabled.
        state
            .settings
            .lock()
            .expect("settings lock")
            .discovery
            .advertise = false;
        let response = handle_comm_capabilities(8, &state);
        assert!(response.ok, "capabilities failed: {:?}", response.error);
        let result = response.result.expect("capabilities result");
        let capabilities = result["capabilities"]
            .as_array()
            .expect("capabilities array");
        let discovery = capabilities
            .iter()
            .find(|entry| entry["id"] == json!("discovery"))
            .expect("discovery entry");
        assert_eq!(discovery["enabled"], json!(true));
        assert_eq!(discovery["announcing"], json!(false));
    }
}
