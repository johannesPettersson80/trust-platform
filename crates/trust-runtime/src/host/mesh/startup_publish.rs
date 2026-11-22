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
use crate::runtime_cloud::contracts::{IdentityPayload, RUNTIME_CLOUD_API_VERSION};
use crate::runtime_cloud::keyspace::{
    meta_identity_key, meta_shm_channels_key, svc_liveliness_key,
};
use crate::runtime_id::platform;
use crate::scheduler::{ResourceCommand, ResourceControl, StdClock};

use super::mapping::{
    build_mesh_payload, decode_mesh_payload, liveliness_peer, mesh_data_key, mesh_liveliness_expr,
    mesh_qos_profile_for_key, now_ns, parse_subscribe_mapping, peer_data_key, snapshot_globals,
};
use super::models::{
    MeshIdentityReply, MeshPeerRegistry, MeshQosProfile, MeshReadiness, MeshService,
};
use super::version::validate_zenoh_version_policy;

const MESH_PUBLISH_INTERVAL: StdDuration = StdDuration::from_millis(1000);
const LIVELINESS_HISTORY_LIMIT: usize = 128;

/// Starts the runtime's mesh. `name` keys its queryables and its identity answer; `stable_runtime_id`
/// is the runtime ID that its announcement and `status` give (specification 11, "Sources").
pub fn start_mesh(
    config: &MeshConfig,
    name: SmolStr,
    stable_runtime_id: SmolStr,
    resource: ResourceControl<StdClock>,
) -> Result<Option<MeshService>, RuntimeError> {
    start_mesh_with_tls(config, name, stable_runtime_id, resource, None)
}

/// Starts the mesh with the runtime's already-loaded TLS material; `name` and `stable_runtime_id`
/// are as for [`start_mesh`].
pub fn start_mesh_with_tls(
    config: &MeshConfig,
    name: SmolStr,
    stable_runtime_id: SmolStr,
    resource: ResourceControl<StdClock>,
    tls: Option<&crate::security::TlsMaterials>,
) -> Result<Option<MeshService>, RuntimeError> {
    if !config.enabled {
        return Ok(None);
    }
    validate_zenoh_version_policy(config)?;

    let listen = config.listen.clone();
    // The runtime's keys are under its own site, and it names its peers relative to that site
    // (specification 11, "Runtime Cloud sites").
    let own_site = SmolStr::new(crate::runtime_cloud::sites::runtime_cloud_site_or_default(
        config.site.as_deref(),
    ));
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
    let authentication = super::security::configure(&mut zenoh_config, config, tls)
        .map_err(|message| RuntimeError::InvalidConfig(message.into()))?;
    let opened = zenoh::open(std::mem::take(&mut zenoh_config)).wait();
    drop(authentication);
    let session = match opened {
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
                site: own_site,
                peer_registry,
                stop_flag,
                publisher_thread: None,
                session: None,
                liveliness_token: None,
            }));
        }
    };

    let liveliness_key = svc_liveliness_key(own_site.as_str(), name.as_str(), name.as_str());
    let liveliness_token = session
        .liveliness()
        .declare_token(liveliness_key.as_str())
        .wait()
        .ok();
    let liveliness_ready = liveliness_token.is_some();

    let liveliness_registry = peer_registry.clone();
    let local_runtime_id = name.clone();
    let liveliness_site = own_site.clone();
    let liveliness_subscriber_ready = session
        .liveliness()
        .declare_subscriber(mesh_liveliness_expr())
        .history(true)
        .callback(move |sample| {
            let Some(peer) = liveliness_peer(sample.key_expr().as_str(), liveliness_site.as_str())
            else {
                return;
            };
            if peer == local_runtime_id.as_str() {
                return;
            }
            let joined = sample.kind() == SampleKind::Put;
            if let Ok(mut guard) = liveliness_registry.lock() {
                guard.record(peer.as_str(), joined, now_ns());
            }
        })
        .background()
        .wait()
        .is_ok();

    let (identity_queryable_ready, catalog_queryable_ready) =
        declare_identity_and_catalog_queryables(
            &session,
            own_site.as_str(),
            name.as_str(),
            stable_runtime_id.as_str(),
            config,
            &resource,
        );
    let _shm_queryable_ready =
        declare_meta_shm_queryable(&session, own_site.as_str(), name.as_str());
    let _cfg_queryable_ready = declare_cfg_queryable(&session, own_site.as_str(), name.as_str());
    let _cmd_queryable_ready = declare_cmd_queryable(&session, own_site.as_str(), name.as_str());
    declare_subscribers(&session, own_site.as_str(), &resource, config);
    super::hmi_reads::declare(
        &session,
        own_site.as_str(),
        name.as_str(),
        &config.publish,
        &resource,
    );
    super::hmi_operations::install(
        &session,
        own_site.as_str(),
        name.as_str(),
        config,
        &resource,
        peer_registry.clone(),
    );

    let publisher_thread = spawn_publish_loop(
        &session,
        own_site.as_str(),
        name.as_str(),
        resource.clone(),
        config.publish.clone(),
        peer_registry.clone(),
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
        site: own_site,
        peer_registry,
        stop_flag,
        publisher_thread,
        session: Some(session),
        liveliness_token,
    }))
}

pub(super) fn build_zenoh_config(config: &MeshConfig) -> Result<ZenohConfig, String> {
    let mut zenoh_config = ZenohConfig::default();
    zenoh_config
        .insert_json5("mode", &format!("\"{}\"", mesh_role_text(config.role)))
        .map_err(|error| format!("mode set failed: {error}"))?;
    // Specification 11, "A mesh links only to the peers it names": Zenoh's scouting is on by
    // default, and would link the mesh to every mesh that answers its multicast on the network and
    // to every peer that a linked peer knows.
    for scouting in ["scouting/multicast/enabled", "scouting/gossip/enabled"] {
        zenoh_config
            .insert_json5(scouting, "false")
            .map_err(|error| format!("{scouting} set failed: {error}"))?;
    }

    if !config.connect.is_empty() {
        let endpoints = config
            .connect
            .iter()
            .map(|endpoint| normalize_endpoint(endpoint.as_str(), config.tls))
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
        let listen = normalize_endpoint(config.listen.as_str(), config.tls)?;
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

/// The Zenoh locator of the mesh endpoint `endpoint`: `host:port`, the URL form `tcp://host:port` or
/// `tls://host:port` that a control endpoint also takes, or a Zenoh locator `tcp/host:port` or
/// `tls/host:port` (specification 11, "Runtime-to-runtime mesh"). With `tls`, a `tcp` endpoint
/// becomes `tls`, and another protocol is refused.
pub(super) fn normalize_endpoint(endpoint: &str, tls: bool) -> Result<String, String> {
    let endpoint = endpoint.trim();
    let canonical = if let Some(address) = endpoint.strip_prefix("tcp://") {
        format!("tcp/{address}")
    } else if let Some(address) = endpoint.strip_prefix("tls://") {
        format!("tls/{address}")
    } else if endpoint.contains('/') {
        endpoint.to_string()
    } else {
        format!("{}/{endpoint}", if tls { "tls" } else { "tcp" })
    };
    if canonical.is_empty() {
        return Err("empty endpoint".to_string());
    }
    if tls {
        if let Some(address) = canonical.strip_prefix("tcp/") {
            return Ok(format!("tls/{address}"));
        }
        if !canonical.starts_with("tls/") {
            return Err("mesh TLS requires TLS endpoints".into());
        }
    }
    Ok(canonical)
}

fn declare_identity_and_catalog_queryables(
    session: &zenoh::Session,
    own_site: &str,
    runtime_id: &str,
    stable_runtime_id: &str,
    config: &MeshConfig,
    resource: &ResourceControl<StdClock>,
) -> (bool, bool) {
    let identity_key = meta_identity_key(own_site, runtime_id);
    // The mesh names a runtime by the resource name that keys its queryables, so that name is also
    // the name in its identity answer (specification 11, "Sources").
    let identity_payload = serde_json::to_string(&MeshIdentityReply {
        identity: IdentityPayload {
            api_version: RUNTIME_CLOUD_API_VERSION.to_string(),
            runtime_id: runtime_id.to_string(),
            site: own_site.to_string(),
            catalog_epoch: 1,
            build: "runtime-cloud-mesh".to_string(),
            capabilities: vec![
                format!("mesh_role={}", config.role.as_str()),
                format!("zenoh_baseline={}", crate::mesh::ZENOH_BASELINE_VERSION),
                format!("zenohd_baseline={}", crate::mesh::ZENOHD_BASELINE_VERSION),
            ],
        },
        name: Some(runtime_id.to_string()),
        version: Some(env!("CARGO_PKG_VERSION").to_string()),
        platform: Some(platform().to_string()),
        stable_runtime_id: Some(stable_runtime_id.to_string()),
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

    let catalog_ready =
        super::catalogue::declare(session, own_site, runtime_id, &config.publish, resource);

    (identity_ready, catalog_ready)
}

fn declare_meta_shm_queryable(session: &zenoh::Session, own_site: &str, runtime_id: &str) -> bool {
    let key = meta_shm_channels_key(own_site, runtime_id);
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

fn declare_cfg_queryable(session: &zenoh::Session, own_site: &str, runtime_id: &str) -> bool {
    let key = format!("truST/{own_site}/{runtime_id}/cfg/**");
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

fn declare_cmd_queryable(session: &zenoh::Session, own_site: &str, runtime_id: &str) -> bool {
    let key = format!("truST/{own_site}/{runtime_id}/cmd/**");
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

/// Subscribes each `peer:key` of `config.subscribe`, `peer` named as the runtime of the site
/// `own_site` names its peers (specification 11, "Runtime Cloud sites").
fn declare_subscribers(
    session: &zenoh::Session,
    own_site: &str,
    resource: &ResourceControl<StdClock>,
    config: &MeshConfig,
) {
    for (remote, local) in &config.subscribe {
        let Some((peer, remote_key)) = parse_subscribe_mapping(remote.as_str()) else {
            continue;
        };
        let key_expr = peer_data_key(own_site, peer.as_str(), remote_key.as_str());
        let local_name = local.clone();
        let resource = resource.clone();
        let _ = session
            .declare_subscriber(key_expr.as_str())
            .callback(move |sample| {
                let templates = match snapshot_globals(&resource, std::slice::from_ref(&local_name))
                {
                    Ok(templates) => templates,
                    Err(err) => {
                        tracing::warn!("mesh subscribe skipped update: {err}");
                        return;
                    }
                };
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
                // The subscriber callback never blocks: a refused update is skipped and logged.
                if let Err(err) = resource.send_command(ResourceCommand::MeshApply {
                    updates,
                    source,
                    sequence,
                }) {
                    tracing::warn!("mesh subscribe skipped update: {err}");
                }
            })
            .background()
            .wait();
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the session, the runtime's site and name, its program, its publish list, its peers and the loop's flags are one publisher"
)]
fn spawn_publish_loop(
    session: &zenoh::Session,
    own_site: &str,
    runtime_id: &str,
    resource: ResourceControl<StdClock>,
    publish: Vec<SmolStr>,
    peers: Arc<Mutex<MeshPeerRegistry>>,
    stop_flag: Arc<AtomicBool>,
    sequence: Arc<AtomicU64>,
) -> Option<thread::JoinHandle<()>> {
    let session = session.clone();
    let own_site = own_site.to_string();
    let runtime_id = runtime_id.to_string();
    Some(thread::spawn(move || {
        while !stop_flag.load(Ordering::Relaxed) {
            let known = peers
                .lock()
                .map(|peers| {
                    peers
                        .peers
                        .iter()
                        .filter(|peer| peer.as_str() != runtime_id)
                        .cloned()
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default();
            super::hmi_reads::poll(&session, own_site.as_str(), &resource, &known);
            if publish.is_empty() {
                thread::sleep(MESH_PUBLISH_INTERVAL);
                continue;
            }
            let snapshot = match snapshot_globals(&resource, &publish) {
                Ok(snapshot) => snapshot,
                Err(err) => {
                    tracing::warn!("mesh publish skipped snapshot: {err}");
                    thread::sleep(MESH_PUBLISH_INTERVAL);
                    continue;
                }
            };
            for (name, value) in snapshot {
                let sequence_value = sequence.fetch_add(1, Ordering::Relaxed);
                let key_expr = mesh_data_key(own_site.as_str(), runtime_id.as_str(), name.as_str());
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
