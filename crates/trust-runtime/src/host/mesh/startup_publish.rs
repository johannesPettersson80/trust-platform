use std::collections::BTreeMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration as StdDuration;

use indexmap::IndexMap;
use smol_str::SmolStr;
use zenoh::qos::{CongestionControl, Priority};
use zenoh::sample::SampleKind;
use zenoh::{Config as ZenohConfig, Wait};

use crate::config::{MeshConfig, MeshRole};
use crate::error::RuntimeError;
use crate::runtime_cloud::contracts::{CatalogEntry, IdentityPayload, RUNTIME_CLOUD_API_VERSION};
use crate::runtime_cloud::keyspace::{
    meta_catalog_key, meta_identity_key, meta_shm_channels_key, svc_liveliness_key,
};
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

use super::mapping::{
    build_mesh_payload, decode_mesh_payload, mesh_data_key, mesh_liveliness_expr,
    mesh_qos_profile_for_key, now_ns, parse_subscribe_mapping, snapshot_globals, DEFAULT_SITE,
};
use super::models::{MeshPeerRegistry, MeshQosProfile, MeshReadiness, MeshService};
use super::version::validate_zenoh_version_policy;

const MESH_PUBLISH_INTERVAL: StdDuration = StdDuration::from_millis(1000);
const LIVELINESS_HISTORY_LIMIT: usize = 128;

pub fn start_mesh(
    config: &MeshConfig,
    name: SmolStr,
    resource: ResourceControl<StdClock>,
) -> Result<Option<MeshService>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    validate_zenoh_version_policy(config)?;

    let listen = config.listen.clone();
    let peer_registry = Arc::new(Mutex::new(MeshPeerRegistry {
        peers: Default::default(),
        history: Default::default(),
        history_limit: LIVELINESS_HISTORY_LIMIT,
    }));
    let stop_flag = Arc::new(AtomicBool::new(false));
    let sequence = Arc::new(AtomicU64::new(1));

    let mut zenoh_config = build_zenoh_config(config).map_err(|message| {
        RuntimeError::InvalidConfig(format!("runtime.mesh configuration error: {message}").into())
    })?;
    let session = match zenoh::open(std::mem::take(&mut zenoh_config)).wait() {
        Ok(session) => session,
        Err(error) => {
            return Ok(Some(MeshService {
                role: config.role,
                listen,
                readiness: MeshReadiness {
                    session_established: false,
                    liveliness_ready: false,
                    identity_queryable_ready: false,
                    catalog_queryable_ready: false,
                },
                degraded_reason: Some(SmolStr::new(format!("zenoh session unavailable: {error}"))),
                peer_registry,
                stop_flag,
                publisher_thread: None,
                session: None,
                liveliness_token: None,
            }));
        }
    };

    let liveliness_key = svc_liveliness_key(DEFAULT_SITE, name.as_str(), name.as_str());
    let liveliness_token = session
        .liveliness()
        .declare_token(liveliness_key.as_str())
        .wait()
        .ok();
    let liveliness_ready = liveliness_token.is_some();

    let liveliness_registry = peer_registry.clone();
    let liveliness_subscriber_ready = session
        .liveliness()
        .declare_subscriber(mesh_liveliness_expr())
        .callback(move |sample| {
            let Some(runtime_id) = liveliness_runtime_id(sample.key_expr().as_str()) else {
                return;
            };
            let joined = sample.kind() == SampleKind::Put;
            if let Ok(mut guard) = liveliness_registry.lock() {
                guard.record(runtime_id, joined, now_ns());
            }
        })
        .background()
        .wait()
        .is_ok();

    let (identity_queryable_ready, catalog_queryable_ready) =
        declare_identity_and_catalog_queryables(&session, name.as_str(), config);
    let _shm_queryable_ready = declare_meta_shm_queryable(&session, name.as_str());
    let _cfg_queryable_ready = declare_cfg_queryable(&session, name.as_str());
    let _cmd_queryable_ready = declare_cmd_queryable(&session, name.as_str());
    declare_subscribers(&session, &resource, config);

    let publisher_thread = spawn_publish_loop(
        &session,
        name.as_str(),
        resource.clone(),
        config.publish.clone(),
        stop_flag.clone(),
        sequence.clone(),
    );

    let readiness = MeshReadiness {
        session_established: true,
        liveliness_ready: liveliness_ready && liveliness_subscriber_ready,
        identity_queryable_ready,
        catalog_queryable_ready,
    };
    let degraded_reason = (!readiness.cloud_ready()).then(|| {
        SmolStr::new("cloud mesh ready requires session + liveliness + identity/catalog queryables")
    });

    Ok(Some(MeshService {
        role: config.role,
        listen,
        readiness,
        degraded_reason,
        peer_registry,
        stop_flag,
        publisher_thread,
        session: Some(session),
        liveliness_token,
    }))
}

fn build_zenoh_config(config: &MeshConfig) -> Result<ZenohConfig, String> {
    let mut zenoh_config = ZenohConfig::default();
    zenoh_config
        .insert_json5("mode", &format!("\"{}\"", mesh_role_text(config.role)))
        .map_err(|error| format!("mode set failed: {error}"))?;

    if !config.connect.is_empty() {
        let endpoints = config
            .connect
            .iter()
            .map(|endpoint| normalize_endpoint(endpoint.as_str()))
            .collect::<Result<Vec<_>, _>>()?;
        zenoh_config
            .insert_json5(
                "connect/endpoints",
                serde_json::to_string(&endpoints)
                    .map_err(|error| format!("serialize connect endpoints: {error}"))?
                    .as_str(),
            )
            .map_err(|error| format!("connect endpoints failed: {error}"))?;
    }

    if matches!(config.role, MeshRole::Peer | MeshRole::Router) {
        let listen = normalize_endpoint(config.listen.as_str())?;
        zenoh_config
            .insert_json5(
                "listen/endpoints",
                serde_json::to_string(&vec![listen])
                    .map_err(|error| format!("serialize listen endpoint: {error}"))?
                    .as_str(),
            )
            .map_err(|error| format!("listen endpoint failed: {error}"))?;
    }

    Ok(zenoh_config)
}

fn mesh_role_text(role: MeshRole) -> &'static str {
    match role {
        MeshRole::Peer => "peer",
        MeshRole::Client => "client",
        MeshRole::Router => "router",
    }
}

fn normalize_endpoint(endpoint: &str) -> Result<String, String> {
    let endpoint = endpoint.trim();
    let canonical = if endpoint.contains('/') {
        endpoint.to_string()
    } else {
        format!("tcp/{endpoint}")
    };
    if canonical.is_empty() {
        return Err("empty endpoint".to_string());
    }
    Ok(canonical)
}

fn declare_identity_and_catalog_queryables(
    session: &zenoh::Session,
    runtime_id: &str,
    config: &MeshConfig,
) -> (bool, bool) {
    let identity_key = meta_identity_key(DEFAULT_SITE, runtime_id);
    let identity_payload = serde_json::to_string(&IdentityPayload {
        api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
        runtime_id: runtime_id.to_string(),
        site: DEFAULT_SITE.to_string(),
        catalog_epoch: 1,
        build: "runtime-cloud-mesh".to_string(),
        capabilities: vec![
            format!("mesh_role={}", config.role.as_str()),
            format!("zenoh_baseline={}", crate::mesh::ZENOH_BASELINE_VERSION),
            format!("zenohd_baseline={}", crate::mesh::ZENOHD_BASELINE_VERSION),
        ],
    })
    .unwrap_or_else(|_| "{}".to_string());
    let identity_ready = session
        .declare_queryable(identity_key.as_str())
        .callback(move |query| {
            let _ = query
                .reply(query.key_expr().clone(), identity_payload.clone())
                .wait();
        })
        .background()
        .wait()
        .is_ok();

    let catalog_key = meta_catalog_key(DEFAULT_SITE, runtime_id);
    let catalog_payload = serde_json::to_string(
        &config
            .publish
            .iter()
            .map(|entry| CatalogEntry {
                api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
                schema_id: entry.to_string(),
                schema_version: 1,
                schema_hash: "sha256:mesh-placeholder".to_string(),
                encoding: "json".to_string(),
                qos: "mesh_t1_fast".to_string(),
            })
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());
    let catalog_ready = session
        .declare_queryable(catalog_key.as_str())
        .callback(move |query| {
            let _ = query
                .reply(query.key_expr().clone(), catalog_payload.clone())
                .wait();
        })
        .background()
        .wait()
        .is_ok();

    (identity_ready, catalog_ready)
}

fn declare_meta_shm_queryable(session: &zenoh::Session, runtime_id: &str) -> bool {
    let key = meta_shm_channels_key(DEFAULT_SITE, runtime_id);
    let payload = serde_json::to_string(&BTreeMap::<String, String>::new())
        .unwrap_or_else(|_| "{}".to_string());
    session
        .declare_queryable(key.as_str())
        .callback(move |query| {
            let _ = query
                .reply(query.key_expr().clone(), payload.clone())
                .wait();
        })
        .background()
        .wait()
        .is_ok()
}

fn declare_cfg_queryable(session: &zenoh::Session, runtime_id: &str) -> bool {
    let key = format!("truST/{}/{}/cfg/**", DEFAULT_SITE, runtime_id);
    let payload = "{\"ok\":false,\"reason\":\"read-only via desired/apply contracts\"}";
    session
        .declare_queryable(key.as_str())
        .callback(move |query| {
            let _ = query.reply(query.key_expr().clone(), payload).wait();
        })
        .background()
        .wait()
        .is_ok()
}

fn declare_cmd_queryable(session: &zenoh::Session, runtime_id: &str) -> bool {
    let key = format!("truST/{}/{}/cmd/**", DEFAULT_SITE, runtime_id);
    let payload = "{\"ok\":false,\"reason\":\"command endpoint unavailable in direct mesh query\"}";
    session
        .declare_queryable(key.as_str())
        .callback(move |query| {
            let _ = query.reply(query.key_expr().clone(), payload).wait();
        })
        .background()
        .wait()
        .is_ok()
}

fn declare_subscribers(
    session: &zenoh::Session,
    resource: &ResourceControl<StdClock>,
    config: &MeshConfig,
) {
    for (remote, local) in &config.subscribe {
        let Some((peer, remote_key)) = parse_subscribe_mapping(remote.as_str()) else {
            continue;
        };
        let key_expr = mesh_data_key(DEFAULT_SITE, peer.as_str(), remote_key.as_str());
        let local_name = local.clone();
        let resource = resource.clone();
        let _ = session
            .declare_subscriber(key_expr.as_str())
            .callback(move |sample| {
                let templates = snapshot_globals(&resource, std::slice::from_ref(&local_name));
                let Some(template) = templates.get(&local_name) else {
                    return;
                };
                let payload = sample.payload().to_bytes();
                let Some((value, source, sequence)) =
                    decode_mesh_payload(payload.as_ref(), template)
                else {
                    return;
                };
                let mut updates = IndexMap::new();
                updates.insert(local_name.clone(), value);
                let _ = resource.send_command(ResourceCommand::MeshApply {
                    updates,
                    source,
                    sequence,
                });
            })
            .background()
            .wait();
    }
}

fn spawn_publish_loop(
    session: &zenoh::Session,
    runtime_id: &str,
    resource: ResourceControl<StdClock>,
    publish: Vec<SmolStr>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
) -> Option<thread::JoinHandle<()>> {
    if publish.is_empty() {
        return None;
    }
    let session = session.clone();
    let runtime_id = runtime_id.to_string();
    Some(thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            let snapshot = snapshot_globals(&resource, &publish);
            for (name, value) in snapshot {
                let sequence_value = sequence.fetch_add(1, Ordering::Relaxed);
                let key_expr = mesh_data_key(DEFAULT_SITE, runtime_id.as_str(), name.as_str());
                let Some(payload) = build_mesh_payload(runtime_id.as_str(), sequence_value, &value)
                else {
                    continue;
                };
                let profile = mesh_qos_profile_for_key(key_expr.as_str());
                let mut put = session.put(key_expr.as_str(), payload);
                put = put
                    .priority(mesh_priority(profile))
                    .congestion_control(mesh_congestion(profile));
                let _ = put.wait();
            }
            thread::sleep(MESH_PUBLISH_INTERVAL);
        }
    }))
}

fn mesh_priority(profile: MeshQosProfile) -> Priority {
    match profile {
        MeshQosProfile::Active => Priority::RealTime,
        MeshQosProfile::Config => Priority::InteractiveHigh,
        MeshQosProfile::Diagnostics => Priority::Background,
        MeshQosProfile::Fast => Priority::DataHigh,
    }
}

fn mesh_congestion(profile: MeshQosProfile) -> CongestionControl {
    match profile {
        MeshQosProfile::Active | MeshQosProfile::Config => CongestionControl::Block,
        MeshQosProfile::Diagnostics | MeshQosProfile::Fast => CongestionControl::Drop,
    }
}

fn liveliness_runtime_id(key_expr: &str) -> Option<&str> {
    key_expr
        .rsplit('/')
        .next()
        .filter(|value| !value.is_empty())
}
