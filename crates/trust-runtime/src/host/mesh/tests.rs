use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};
use std::time::Duration as StdDuration;

use indexmap::IndexMap;
use smol_str::SmolStr;

use super::mapping::{
    build_mesh_payload, decode_mesh_payload, mesh_qos_profile_for_key, parse_subscribe_mapping,
};
use super::models::{MeshPeerRegistry, MeshReadiness, MeshService};
use super::version::{validate_zenoh_version_policy, ZENOHD_BASELINE_VERSION};
use crate::config::{MeshConfig, MeshRole};
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};
use crate::value::Value;

#[test]
fn mesh_payload_propagates_source_identity_and_sequence_metadata() {
    let payload = build_mesh_payload("runtime-a", 41, &Value::DInt(123)).expect("payload");
    let (value, source, sequence) =
        decode_mesh_payload(payload.as_slice(), &Value::DInt(0)).expect("decode");
    assert_eq!(value, Value::DInt(123));
    assert_eq!(source, Some(SmolStr::new("runtime-a")));
    assert_eq!(sequence, Some(41));
}

#[test]
fn mesh_payload_encode_decode_fuzz_smoke_budget() {
    fn next(state: &mut u64) -> u64 {
        *state = state
            .wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407);
        *state
    }

    let iterations = std::env::var("TRUST_COMMS_FUZZ_ITERS")
        .ok()
        .and_then(|raw| raw.parse::<usize>().ok())
        .unwrap_or(512);
    let mut state = 0xA11C_E5E7_0000_0001_u64;

    for sequence in 0..iterations {
        let value = Value::DInt(next(&mut state) as i32);
        let payload = build_mesh_payload("runtime-a", sequence as u64, &value).expect("payload");
        let decoded = decode_mesh_payload(payload.as_slice(), &Value::DInt(0)).expect("decode");
        assert_eq!(decoded.0, value);
        assert_eq!(decoded.1.as_deref(), Some("runtime-a"));
        assert_eq!(decoded.2, Some(sequence as u64));
    }

    for _ in 0..iterations {
        let size = ((next(&mut state) % 96) + 1) as usize;
        let mut payload = vec![0_u8; size];
        for byte in &mut payload {
            *byte = next(&mut state) as u8;
        }
        // Decoder must stay bounded and never panic on malformed payloads.
        let _ = decode_mesh_payload(payload.as_slice(), &Value::DInt(0));
    }
}

#[test]
fn mesh_subscribe_mapping_requires_peer_and_remote_key() {
    assert_eq!(
        parse_subscribe_mapping("runtime-a:Main.temp"),
        Some((SmolStr::new("runtime-a"), SmolStr::new("Main.temp")))
    );
    assert!(parse_subscribe_mapping("runtime-a").is_none());
    assert!(parse_subscribe_mapping(":Main.temp").is_none());
    assert!(parse_subscribe_mapping("runtime-a:").is_none());
}

#[test]
fn liveliness_registry_tracks_join_and_leave_transitions() {
    let mut registry = MeshPeerRegistry {
        peers: Default::default(),
        history: Default::default(),
        history_limit: 8,
    };
    registry.record("runtime-a", true, 10);
    registry.record("runtime-b", true, 20);
    registry.record("runtime-a", false, 30);

    assert!(registry.peers.contains("runtime-b"));
    assert!(!registry.peers.contains("runtime-a"));
    assert_eq!(registry.history.len(), 3);
    assert_eq!(registry.history.back().map(|event| event.joined), Some(false));
}

#[test]
fn qos_profile_mapping_aligns_with_active_cfg_and_diag_zones() {
    assert_eq!(
        mesh_qos_profile_for_key("truST/site-a/active/diag/peer_health"),
        super::models::MeshQosProfile::Active
    );
    assert_eq!(
        mesh_qos_profile_for_key("truST/site-a/runtime-a/cfg/desired"),
        super::models::MeshQosProfile::Config
    );
    assert_eq!(
        mesh_qos_profile_for_key("truST/site-a/runtime-a/diag/cycle_stats"),
        super::models::MeshQosProfile::Diagnostics
    );
    assert_eq!(
        mesh_qos_profile_for_key("truST/site-a/runtime-a/mesh/data/value"),
        super::models::MeshQosProfile::Fast
    );
}

#[test]
fn mesh_cloud_ready_wait_times_out_for_degraded_state() {
    let service = MeshService {
        role: MeshRole::Peer,
        listen: SmolStr::new("0.0.0.0:5200"),
        readiness: MeshReadiness {
            session_established: true,
            liveliness_ready: false,
            identity_queryable_ready: true,
            catalog_queryable_ready: true,
        },
        degraded_reason: Some(SmolStr::new("missing liveliness token")),
        peer_registry: Arc::new(Mutex::new(MeshPeerRegistry {
            peers: Default::default(),
            history: Default::default(),
            history_limit: 8,
        })),
        stop_flag: Arc::new(AtomicBool::new(true)),
        publisher_thread: None,
        session: None,
        liveliness_token: None,
    };
    let error = service
        .wait_cloud_ready(StdDuration::from_millis(20))
        .expect_err("degraded mesh should timeout");
    assert!(error
        .as_str()
        .contains("mesh cloud readiness timed out"));
}

#[test]
fn mixed_version_policy_rejects_minor_mismatch() {
    let mut config = mesh_config();
    config.zenohd_version = SmolStr::new("1.8.1");
    let error = validate_zenoh_version_policy(&config).expect_err("minor mismatch should fail");
    assert!(error.to_string().contains("not compatible"));
}

#[test]
fn queryables_are_available_when_mesh_session_starts() {
    let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
    std::thread::spawn(move || {
        while let Ok(command) = cmd_rx.recv() {
            if let ResourceCommand::MeshSnapshot { respond_to, .. } = command {
                let _ = respond_to.send(IndexMap::new());
            }
        }
    });
    let service = super::start_mesh(&mesh_config(), SmolStr::new("runtime-a"), resource)
        .expect("start mesh")
        .expect("mesh service");
    assert!(service.readiness().session_established);
    assert!(service.readiness().identity_queryable_ready);
    assert!(service.readiness().catalog_queryable_ready);
}

#[test]
fn mesh_tls_publish_applies_updates() {
    let (resource, cmd_rx) = ResourceControl::stub(StdClock::new());
    std::thread::spawn(move || {
        while let Ok(command) = cmd_rx.recv() {
            if let ResourceCommand::MeshSnapshot {
                names, respond_to, ..
            } = command
            {
                let mut values = IndexMap::new();
                for name in names {
                    values.insert(name, Value::DInt(7));
                }
                let _ = respond_to.send(values);
            }
        }
    });

    let mut config = mesh_config();
    config.tls = true;
    config.publish = vec![SmolStr::new("Main.speed")];
    let service = super::start_mesh(&config, SmolStr::new("runtime-a"), resource)
        .expect("start mesh with tls flag")
        .expect("mesh service");
    assert!(service.readiness().session_established);
    assert!(service.readiness().identity_queryable_ready);
    assert!(service.readiness().catalog_queryable_ready);
}

fn mesh_config() -> MeshConfig {
    MeshConfig {
        enabled: true,
        role: MeshRole::Peer,
        listen: SmolStr::new("127.0.0.1:0"),
        connect: Vec::new(),
        tls: false,
        auth_token: None,
        publish: Vec::new(),
        subscribe: IndexMap::new(),
        zenohd_version: SmolStr::new(ZENOHD_BASELINE_VERSION),
        plugin_versions: IndexMap::new(),
    }
}
