//! HMI credentials and commands require both existing mesh authentication and TLS.
use super::models::MeshPeerRegistry;
use crate::config::MeshConfig;
use crate::scheduler::{ResourceControl, StdClock};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zenoh::{Session, Wait};

/// The HMI operation key of the runtime `runtime` of the site `site`.
fn key(site: &str, runtime: &str) -> String {
    format!("truST/{site}/{runtime}/hmi/operate")
}
pub(super) fn install(
    session: &Session,
    own_site: &str,
    runtime: &str,
    config: &MeshConfig,
    resource: &ResourceControl<StdClock>,
    peers: Arc<Mutex<MeshPeerRegistry>>,
) {
    let exchange = resource.hmi_exchange();
    exchange
        .remote_operations
        .publish(config.publish.iter().map(ToString::to_string).collect());
    if !config.tls || config.auth_token.is_none() {
        return;
    }
    let owner = Arc::downgrade(&exchange);
    let _ = session
        .declare_queryable(key(own_site, runtime))
        .callback(move |query| {
            let answer = (|| {
                let bytes = query.payload()?.to_bytes();
                if bytes.len() > 16 * 1024 {
                    return None;
                }
                let request: Value = serde_json::from_slice(&bytes).ok()?;
                Some(owner.upgrade()?.remote_operations.receive(&request))
            })()
            .unwrap_or_else(
                || json!({"ok":false,"error":"invalid or unavailable remote HMI request"}),
            );
            let _ = query
                .reply(query.key_expr().clone(), answer.to_string())
                .wait();
        })
        .background()
        .wait();
    let session = session.clone();
    // A peer is named as this runtime names its peers, relative to its own site (specification
    // 11, "Runtime Cloud sites").
    let own_site = own_site.to_string();
    exchange
        .remote_operations
        .transport(Arc::new(move |peer, request| {
            if !peers
                .lock()
                .map(|peers| peers.peers.contains(peer))
                .unwrap_or(false)
            {
                return Err("the owning runtime is unreachable".into());
            }
            let (site, name) = super::mapping::peer_site_and_name(peer, own_site.as_str());
            let replies = session
                .get(key(site, name))
                .payload(request.to_string())
                .timeout(Duration::from_secs(4))
                .wait()
                .map_err(|_| "unknown: the owning runtime did not accept a transport receipt")?;
            let reply = replies
                .recv_timeout(Duration::from_secs(4))
                .map_err(|_| {
                    "unknown: the owner did not answer; inspect its state before retrying"
                })?
                .ok_or("unknown: the owner did not answer")?;
            let sample = reply
                .result()
                .map_err(|_| "the owner refused the remote operation")?;
            let bytes = sample.payload().to_bytes();
            if bytes.len() > 64 * 1024 {
                return Err("owner response exceeds 64 KiB".into());
            }
            serde_json::from_slice(&bytes).map_err(|_| "invalid owner response".into())
        }));
}
