//! Bounded HMI reads reuse the owning runtime's Hmi.Display exchange and publish allowlist.
use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use serde_json::{json, Value};
use smol_str::SmolStr;
use zenoh::{Session, Wait};

use crate::hmi::commands::write::path_text;
use crate::hmi::member_paths::MemberPath;
use crate::hmi::remote_values::remote_path;
use crate::scheduler::{ResourceControl, StdClock};

const TIMEOUT: Duration = Duration::from_millis(750);
const MAX_REPLY: usize = 2 * 1024 * 1024;
/// The HMI read key of the runtime `runtime` of the site `site`.
fn key(site: &str, runtime: &str) -> String {
    format!("truST/{site}/{runtime}/hmi/read")
}

/// The HMI read key of `peer`, named as a runtime of the site `own_site` names it (specification
/// 11, "Runtime Cloud sites").
fn peer_key(own_site: &str, peer: &str) -> String {
    let (site, name) = super::mapping::peer_site_and_name(peer, own_site);
    key(site, name)
}

pub(super) fn declare(
    session: &Session,
    own_site: &str,
    runtime: &str,
    publish: &[SmolStr],
    resource: &ResourceControl<StdClock>,
) {
    let published: Vec<_> = publish
        .iter()
        .map(|path| crate::hmi::remote_values::qualified_key(path))
        .collect();
    let exchange = resource.hmi_exchange();
    let _ = session
        .declare_queryable(key(own_site, runtime))
        .callback(move |query| {
            let result = (|| -> Result<Vec<u8>, String> {
                let payload = query
                    .payload()
                    .ok_or("missing HMI read request")?
                    .to_bytes();
                if payload.len() > 64 * 1024 {
                    return Err("HMI read request exceeds 64 KiB".into());
                }
                let names: Vec<String> =
                    serde_json::from_slice(&payload).map_err(|_| "invalid HMI read request")?;
                if names.is_empty() || names.len() > 500 {
                    return Err("HMI read requires 1 to 500 published paths".into());
                }
                let paths = names
                    .iter()
                    .map(|name| {
                        if !published.contains(&crate::hmi::remote_values::qualified_key(name)) {
                            return Err(format!("{name} is not published by this runtime"));
                        }
                        let path = MemberPath::parse(name)?;
                        if remote_path(&path).is_some() {
                            return Err(
                                "remote HMI reads cannot forward through another owner".into()
                            );
                        }
                        Ok(path)
                    })
                    .collect::<Result<Vec<_>, String>>()?;
                let read = exchange
                    .read_next(&paths, TIMEOUT)
                    .ok_or("the owner has not called Hmi.Display")?;
                let bytes = serde_json::to_vec(&read).map_err(|error| error.to_string())?;
                if bytes.len() > MAX_REPLY {
                    return Err("HMI read response exceeds 2 MiB".into());
                }
                Ok(bytes)
            })();
            match result {
                Ok(bytes) => {
                    let _ = query.reply(query.key_expr().clone(), bytes).wait();
                }
                Err(reason) => {
                    let _ = query.reply_err(reason).wait();
                }
            }
        })
        .background()
        .wait();
}

/// Called by the existing background mesh worker, never by the runtime task.
pub(super) fn poll(
    session: &Session,
    own_site: &str,
    resource: &ResourceControl<StdClock>,
    known_peers: &[String],
) {
    let exchange = resource.hmi_exchange();
    let paths = exchange.remote_paths();
    if !paths.is_empty() || exchange.remote_values.take_catalogue_request() {
        let deadline = Instant::now() + TIMEOUT;
        let catalogues: Vec<_> = known_peers
            .iter()
            .take(32)
            .filter_map(|peer| {
                let (site, name) = super::mapping::peer_site_and_name(peer, own_site);
                session
                    .get(crate::runtime_cloud::keyspace::meta_catalog_key(site, name))
                    .timeout(TIMEOUT)
                    .wait()
                    .ok()
                    .map(|replies| (peer, replies))
            })
            .collect();
        for (peer, replies) in catalogues {
            let Ok(Some(reply)) =
                replies.recv_timeout(deadline.saturating_duration_since(Instant::now()))
            else {
                continue;
            };
            let Ok(sample) = reply.result() else {
                continue;
            };
            let bytes = sample.payload().to_bytes();
            if bytes.len() > MAX_REPLY {
                continue;
            }
            if let Ok(entries) = serde_json::from_slice::<Vec<Value>>(&bytes) {
                exchange.remote_values.update_catalogue(peer, entries);
            }
        }
    }
    exchange.remote_values.retain(&paths);
    let mut peers: BTreeMap<String, Vec<(MemberPath, String)>> = BTreeMap::new();
    for path in paths {
        if let Some((peer, local)) = remote_path(&path) {
            peers
                .entry(peer)
                .or_default()
                .push((path, path_text(&local)));
        }
    }
    let deadline = Instant::now() + TIMEOUT;
    let pending: Vec<_> = peers
        .into_iter()
        .take(32)
        .filter_map(|(peer, entries)| {
            let names: Vec<_> = entries.iter().map(|(_, name)| name).collect();
            session
                .get(peer_key(own_site, &peer))
                .payload(json!(names).to_string())
                .timeout(TIMEOUT)
                .wait()
                .ok()
                .map(|replies| (entries, replies))
        })
        .collect();
    for (entries, replies) in pending {
        let Ok(Some(reply)) =
            replies.recv_timeout(deadline.saturating_duration_since(Instant::now()))
        else {
            continue;
        };
        let Ok(sample) = reply.result() else {
            continue;
        };
        let bytes = sample.payload().to_bytes();
        if bytes.len() > MAX_REPLY {
            continue;
        }
        let Ok(answer) = serde_json::from_slice::<Value>(&bytes) else {
            continue;
        };
        let paths: Vec<_> = entries.into_iter().map(|(path, _)| path).collect();
        exchange.remote_values.update(&paths, &answer);
    }
}
