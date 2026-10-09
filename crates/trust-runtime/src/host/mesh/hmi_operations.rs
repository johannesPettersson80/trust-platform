//! HMI credentials and commands require both existing mesh authentication and TLS.
use super::models::MeshPeerRegistry;
use crate::config::MeshConfig;
use crate::scheduler::{ResourceControl, StdClock};
use serde_json::{json, Value};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use zenoh::{Session, Wait};

fn key(runtime: &str) -> String {
    format!("truST/default-site/{runtime}/hmi/operate")
}
pub(super) fn install(
    session: &Session,
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
        .declare_queryable(key(runtime))
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
            let replies = session
                .get(key(peer))
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
