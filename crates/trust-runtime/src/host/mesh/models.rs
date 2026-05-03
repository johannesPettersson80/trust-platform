use std::collections::{BTreeSet, VecDeque};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration as StdDuration, Instant};

use smol_str::SmolStr;
use zenoh::liveliness::LivelinessToken;
use zenoh::{Session, Wait};

use crate::config::MeshRole;

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
