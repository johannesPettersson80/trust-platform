use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use serde::{Deserialize, Serialize};
use smol_str::SmolStr;
use zenoh::liveliness::LivelinessToken;
use zenoh::query::Reply;
use zenoh::{Session, Wait};

use crate::config::MeshRole;
use crate::runtime_cloud::contracts::IdentityPayload;
use crate::runtime_cloud::keyspace::meta_identity_key;

use super::mapping::DEFAULT_SITE;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeshQosProfile {
    Active,
    Config,
    Diagnostics,
    Fast,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLivelinessEvent {
    pub runtime_id: String,
    pub joined: bool,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshLivelinessSnapshot {
    pub peers: Vec<String>,
    pub history: Vec<MeshLivelinessEvent>,
}

/// Live mesh evidence exposed to control-plane topology surfaces.
#[derive(Debug, Clone)]
pub struct MeshTopologyEvidence {
    ready: bool,
    degraded_reason: Option<SmolStr>,
    peer_registry: Arc<Mutex<MeshPeerRegistry>>,
    session: Option<Session>,
}

impl MeshTopologyEvidence {
    #[must_use]
    pub const fn is_ready(&self) -> bool {
        self.ready
    }

    /// The mesh session that answers the identity queries of the peers; `None` when the mesh has
    /// no session.
    #[must_use]
    pub(crate) fn session(&self) -> Option<&Session> {
        self.session.as_ref()
    }

    #[must_use]
    pub fn degraded_reason(&self) -> Option<&str> {
        self.degraded_reason.as_deref()
    }

    #[must_use]
    pub fn liveliness_snapshot(&self) -> MeshLivelinessSnapshot {
        if let Ok(guard) = self.peer_registry.lock() {
            return MeshLivelinessSnapshot {
                peers: guard.peers.iter().cloned().collect(),
                history: guard.history.iter().cloned().collect(),
            };
        }
        MeshLivelinessSnapshot {
            peers: Vec::new(),
            history: Vec::new(),
        }
    }

    #[cfg(test)]
    pub fn for_test(ready: bool, peers: &[&str], timestamp_ns: u64) -> Self {
        let mut registry = MeshPeerRegistry {
            peers: BTreeSet::new(),
            history: VecDeque::new(),
            history_limit: 64,
        };
        for peer in peers {
            registry.record(peer, true, timestamp_ns);
        }
        Self {
            ready,
            degraded_reason: None,
            peer_registry: Arc::new(Mutex::new(registry)),
            session: None,
        }
    }

    /// Evidence with a mesh session, for a search that asks the peers `peers` through it.
    #[cfg(test)]
    pub fn for_test_with_session(session: Session, peers: &[&str]) -> Self {
        Self {
            session: Some(session),
            ..Self::for_test(true, peers, 0)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MeshReadiness {
    pub session_established: bool,
    pub liveliness_ready: bool,
    pub identity_queryable_ready: bool,
    pub catalog_queryable_ready: bool,
}

impl MeshReadiness {
    #[must_use]
    pub fn cloud_ready(&self) -> bool {
        self.session_established
            && self.liveliness_ready
            && self.identity_queryable_ready
            && self.catalog_queryable_ready
    }
}

#[derive(Debug, Default)]
pub(crate) struct MeshPeerRegistry {
    pub peers: BTreeSet<String>,
    pub history: VecDeque<MeshLivelinessEvent>,
    pub history_limit: usize,
}

impl MeshPeerRegistry {
    pub fn record(&mut self, runtime_id: &str, joined: bool, timestamp_ns: u64) {
        if joined {
            self.peers.insert(runtime_id.to_string());
        } else {
            self.peers.remove(runtime_id);
        }
        self.history.push_back(MeshLivelinessEvent {
            runtime_id: runtime_id.to_string(),
            joined,
            timestamp_ns,
        });
        while self.history.len() > self.history_limit {
            self.history.pop_front();
        }
    }
}

pub struct MeshService {
    pub(crate) role: MeshRole,
    pub(crate) listen: SmolStr,
    pub(crate) readiness: MeshReadiness,
    pub(crate) degraded_reason: Option<SmolStr>,
    pub(crate) peer_registry: Arc<Mutex<MeshPeerRegistry>>,
    pub(crate) stop_flag: Arc<AtomicBool>,
    pub(crate) publisher_thread: Option<thread::JoinHandle<()>>,
    pub(crate) session: Option<Session>,
    pub(crate) liveliness_token: Option<LivelinessToken>,
}

impl MeshService {
    #[must_use]
    pub fn is_ready(&self) -> bool {
        self.readiness.cloud_ready()
    }

    #[must_use]
    pub fn readiness(&self) -> &MeshReadiness {
        &self.readiness
    }

    #[must_use]
    pub fn degraded_reason(&self) -> Option<&str> {
        self.degraded_reason.as_deref()
    }

    #[must_use]
    pub fn discovery_mesh_listen(&self) -> Option<&str> {
        if !self.is_ready() {
            return None;
        }
        match self.role {
            MeshRole::Peer | MeshRole::Router => Some(self.listen.as_str()),
            MeshRole::Client => None,
        }
    }

    #[must_use]
    pub fn liveliness_snapshot(&self) -> MeshLivelinessSnapshot {
        if let Ok(guard) = self.peer_registry.lock() {
            return MeshLivelinessSnapshot {
                peers: guard.peers.iter().cloned().collect(),
                history: guard.history.iter().cloned().collect(),
            };
        }
        MeshLivelinessSnapshot {
            peers: Vec::new(),
            history: Vec::new(),
        }
    }

    #[must_use]
    pub fn topology_evidence(&self) -> MeshTopologyEvidence {
        MeshTopologyEvidence {
            ready: self.is_ready(),
            degraded_reason: self.degraded_reason.clone(),
            peer_registry: self.peer_registry.clone(),
            session: self.session.clone(),
        }
    }

    pub fn wait_cloud_ready(&self, timeout: StdDuration) -> Result<(), SmolStr> {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if self.is_ready() {
                return Ok(());
            }
            thread::sleep(StdDuration::from_millis(10));
        }
        Err(SmolStr::new(
            "mesh cloud readiness timed out waiting for liveliness and queryables",
        ))
    }
}

impl Drop for MeshService {
    fn drop(&mut self) {
        self.stop_flag.store(true, Ordering::Relaxed);
        if let Some(handle) = self.publisher_thread.take() {
            let _ = handle.join();
        }
        self.liveliness_token.take();
        if let Some(session) = self.session.take() {
            let _ = session.close().wait();
        }
    }
}

/// What a runtime answers to an identity query (specification 11, "Sources"): the identity that
/// every runtime sends, and the name, version, platform and stable runtime ID that a runtime
/// sending them adds as optional fields. An answer without them is still an identity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct MeshIdentityReply {
    #[serde(flatten)]
    pub(crate) identity: IdentityPayload,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) platform: Option<String>,
    /// The runtime ID that its announcement and `status` give, so that a search merges the answer
    /// with the mDNS find of the same runtime (specification 11, "Sources").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) stable_runtime_id: Option<String>,
}

/// Asks the identity queryable of every peer in `peers` at once, and reads the answers until
/// `timeout` (specification 11, "Sources"). The query only reads, and no peer is written to. A peer
/// that does not answer gives no entry. Each answer gives its peer's entry: the identity it sent,
/// or the reason that the answer is not the identity of the peer that was asked.
pub(crate) fn query_identities(
    session: &Session,
    peers: &[String],
    timeout: StdDuration,
) -> Vec<(String, Result<MeshIdentityReply, String>)> {
    let queries: Vec<_> = peers
        .iter()
        .map(|peer| {
            let key = meta_identity_key(DEFAULT_SITE, peer);
            (peer, session.get(key.as_str()).timeout(timeout).wait())
        })
        .collect();
    let mut answers = Vec::new();
    for (peer, query) in queries {
        match query {
            Ok(replies) => {
                while let Ok(Some(reply)) = replies.recv_timeout(timeout) {
                    answers.push((peer.clone(), decode_identity_answer(peer, reply)));
                }
            }
            Err(error) => answers.push((
                peer.clone(),
                Err(format!("the identity query could not be sent: {error}")),
            )),
        }
    }
    answers
}

fn decode_identity_answer(peer: &str, reply: Reply) -> Result<MeshIdentityReply, String> {
    let sample = reply
        .into_result()
        .map_err(|error| format!("the identity query was refused: {error}"))?;
    let payload = sample.payload().to_bytes();
    let answer: MeshIdentityReply = serde_json::from_slice(&payload)
        .map_err(|error| format!("the identity answer could not be read: {error}"))?;
    if answer.identity.runtime_id != peer {
        return Err(format!(
            "the identity answer names runtime {}, not {peer}",
            answer.identity.runtime_id
        ));
    }
    Ok(answer)
}

#[cfg(test)]
mod identity_answer_tests {
    use super::*;

    fn identity() -> IdentityPayload {
        IdentityPayload {
            api_version: "1.0".to_string(),
            runtime_id: "rt-a".to_string(),
            site: "default-site".to_string(),
            catalog_epoch: 1,
            build: "runtime-cloud-mesh".to_string(),
            capabilities: vec!["mesh_role=peer".to_string()],
        }
    }

    #[test]
    fn identity_answer_gives_its_name_version_platform_and_stable_id_after_the_identity() {
        // Spec 11, "Sources": a runtime that sends its name, version, platform and stable runtime ID
        // gives them as optional fields after its identity. The producer sends the stable ID as
        // `stable_runtime_id`; a wrong name or order breaks every older answer.
        let answer = MeshIdentityReply {
            identity: identity(),
            name: Some("Mixer-02".to_string()),
            version: Some("0.24.75".to_string()),
            platform: Some("linux-x86_64".to_string()),
            stable_runtime_id: Some("3f9c2a1b7e4d5f60a18c2e4d6b9f0a13".to_string()),
        };
        assert_eq!(
            serde_json::to_string(&answer).expect("json"),
            r#"{"api_version":"1.0","runtime_id":"rt-a","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"],"name":"Mixer-02","version":"0.24.75","platform":"linux-x86_64","stable_runtime_id":"3f9c2a1b7e4d5f60a18c2e4d6b9f0a13"}"#
        );
    }

    #[test]
    fn identity_answer_without_the_optional_fields_is_read_as_its_mesh_name_only() {
        // Spec 11, "Sources": an answer that sends no name, version, platform or stable runtime ID is
        // still an identity, named by its mesh name, and it is written back with exactly the six
        // identity fields.
        let older = r#"{"api_version":"1.0","runtime_id":"rt-a","site":"default-site","catalog_epoch":1,"build":"runtime-cloud-mesh","capabilities":["mesh_role=peer"]}"#;
        let answer: MeshIdentityReply =
            serde_json::from_str(older).expect("an older answer parses");
        assert_eq!(answer.identity.runtime_id, "rt-a");
        assert_eq!(answer.name, None);
        assert_eq!(answer.version, None);
        assert_eq!(answer.platform, None);
        assert_eq!(answer.stable_runtime_id, None);
        assert_eq!(
            serde_json::to_string(&answer).expect("json"),
            older,
            "an absent optional field is not written"
        );
    }
}
