//! Which runtimes of the mesh come, go and stay away, as the runtime's log reports them
//! (specification 11, "A mesh says which peers come, go or stay away").

use std::collections::BTreeSet;
use std::sync::{Mutex, Weak};
use std::time::{Duration, Instant};

use indexmap::IndexMap;
use smol_str::SmolStr;

use super::models::MeshPeerRegistry;

/// How long after the mesh starts a runtime that a subscription names may take to join before the
/// runtime says that it is missing.
pub const MESH_PEER_GRACE: Duration = Duration::from_secs(5);

/// A change of the mesh's runtimes that the runtime logs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MeshPeerChange {
    /// A runtime joined the mesh: it linked and declared itself.
    Joined(String),
    /// A runtime left the mesh.
    Left(String),
    /// A runtime that subscriptions name was not in the mesh [`MESH_PEER_GRACE`] after the mesh
    /// started; with those subscriptions, in the order of the configuration.
    Missing {
        peer: String,
        subscriptions: Vec<String>,
    },
}

/// The runtimes of a running mesh, as its liveliness registry knows them. It holds the registry
/// weakly, so it ends with the mesh.
#[derive(Debug, Clone)]
pub struct MeshPeers(pub(super) Weak<Mutex<MeshPeerRegistry>>);

impl MeshPeers {
    /// The runtimes that are in the mesh now, by the names the runtime gives its peers
    /// (specification 11, "Runtime Cloud sites"); `None` once the mesh has stopped.
    #[must_use]
    pub fn current(&self) -> Option<BTreeSet<String>> {
        let registry = self.0.upgrade()?;
        let guard = registry.lock().ok()?;
        Some(guard.peers.iter().cloned().collect())
    }
}

/// Tells the changes of the mesh's runtimes from one look at them to the next: who joined and
/// who left since the last look, and, once, [`MESH_PEER_GRACE`] after the mesh started, every
/// runtime that a subscription names and that is not in the mesh then.
#[derive(Debug)]
pub struct MeshPresenceWatch {
    /// Each runtime that a subscription names, `<peer>:<path>`, with its subscriptions' keys.
    named: IndexMap<String, Vec<String>>,
    started: Instant,
    known: BTreeSet<String>,
    missing_told: bool,
}

impl MeshPresenceWatch {
    /// The watch of a mesh that started at `started` with the subscriptions `subscribe`, keyed by
    /// `<peer>:<path>` (specification 11, "Runtime-to-runtime mesh").
    #[must_use]
    pub fn new(subscribe: &IndexMap<SmolStr, SmolStr>, started: Instant) -> Self {
        let mut named: IndexMap<String, Vec<String>> = IndexMap::new();
        for key in subscribe.keys() {
            if let Some((peer, _)) = key.split_once(':') {
                named
                    .entry(peer.to_string())
                    .or_default()
                    .push(key.to_string());
            }
        }
        Self {
            named,
            started,
            known: BTreeSet::new(),
            missing_told: false,
        }
    }

    /// The changes since the last look, the mesh's runtimes being `peers` at `now`: the runtimes
    /// that joined, then those that left, each in name order, then, at the first look
    /// [`MESH_PEER_GRACE`] or more after the start, the named runtimes that are not in `peers`.
    pub fn changes(&mut self, peers: &BTreeSet<String>, now: Instant) -> Vec<MeshPeerChange> {
        let mut changes: Vec<MeshPeerChange> = peers
            .difference(&self.known)
            .map(|peer| MeshPeerChange::Joined(peer.clone()))
            .collect();
        changes.extend(
            self.known
                .difference(peers)
                .map(|peer| MeshPeerChange::Left(peer.clone())),
        );
        self.known.clone_from(peers);
        if !self.missing_told && now.saturating_duration_since(self.started) >= MESH_PEER_GRACE {
            self.missing_told = true;
            changes.extend(
                self.named
                    .iter()
                    .filter(|(peer, _)| !peers.contains(peer.as_str()))
                    .map(|(peer, subscriptions)| MeshPeerChange::Missing {
                        peer: peer.clone(),
                        subscriptions: subscriptions.clone(),
                    }),
            );
        }
        changes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn peers(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    /// The subscriptions of tutorial 15's PLC-A, with a second one of PLC-B's and one of a runtime
    /// `PlcC`.
    fn subscribe() -> IndexMap<SmolStr, SmolStr> {
        [
            ("PlcB:P1.LampOut", "P1.RemoteLamp"),
            ("PlcC:P1.Level", "P1.RemoteLevel"),
            ("PlcB:P1.Speed", "P1.RemoteSpeed"),
        ]
        .into_iter()
        .map(|(key, target)| (SmolStr::new(key), SmolStr::new(target)))
        .collect()
    }

    #[test]
    fn joins_and_leaves_are_told_once_each_in_name_order() {
        // Specification 11, "A mesh says which peers come, go or stay away": each look tells the
        // runtimes that joined since the last one, then those that left. PlcB and PlcD join, PlcD
        // leaves while PlcA joins, nothing changes, then PlcB leaves. Catches a join told at every
        // look, a leave never told, and the two told in the other order.
        let start = Instant::now();
        let mut watch = MeshPresenceWatch::new(&IndexMap::new(), start);
        assert_eq!(
            watch.changes(&peers(&["PlcD", "PlcB"]), start),
            [
                MeshPeerChange::Joined("PlcB".into()),
                MeshPeerChange::Joined("PlcD".into())
            ]
        );
        assert_eq!(
            watch.changes(&peers(&["PlcA", "PlcB"]), start),
            [
                MeshPeerChange::Joined("PlcA".into()),
                MeshPeerChange::Left("PlcD".into())
            ]
        );
        assert_eq!(watch.changes(&peers(&["PlcA", "PlcB"]), start), []);
        assert_eq!(
            watch.changes(&peers(&["PlcA"]), start),
            [MeshPeerChange::Left("PlcB".into())]
        );
    }

    #[test]
    fn a_named_runtime_still_away_after_the_grace_is_told_missing_once() {
        // Specification 11: a runtime that subscriptions name and that has not joined 5 s after the
        // start is told missing once, with its subscriptions in the order of the configuration.
        // PlcB is away at 4.9 s, nothing is told; at 5 s PlcC has joined and PlcB has not, so PlcB
        // is missing with both of its subscriptions; at 6 s nothing more; PlcB's join at 7 s is a
        // join. Catches a runtime told missing before the grace, again at every later look, one
        // that joined told missing, and its second subscription left out.
        let start = Instant::now();
        let mut watch = MeshPresenceWatch::new(&subscribe(), start);
        let at = |millis: u64| start + Duration::from_millis(millis);
        assert_eq!(watch.changes(&peers(&[]), at(4_900)), []);
        assert_eq!(
            watch.changes(&peers(&["PlcC"]), at(5_000)),
            [
                MeshPeerChange::Joined("PlcC".into()),
                MeshPeerChange::Missing {
                    peer: "PlcB".into(),
                    subscriptions: vec!["PlcB:P1.LampOut".into(), "PlcB:P1.Speed".into()],
                },
            ]
        );
        assert_eq!(watch.changes(&peers(&["PlcC"]), at(6_000)), []);
        assert_eq!(
            watch.changes(&peers(&["PlcB", "PlcC"]), at(7_000)),
            [MeshPeerChange::Joined("PlcB".into())]
        );
    }

    #[test]
    fn a_mesh_without_subscriptions_names_no_runtime_missing() {
        // Specification 11: only a runtime that a subscription names can be missing; a publisher's
        // mesh, as PLC-B's in tutorial 15, waits for nobody. Catches a missing runtime made up from
        // a mesh's own name or its publications.
        let start = Instant::now();
        let mut watch = MeshPresenceWatch::new(&IndexMap::new(), start);
        assert_eq!(watch.changes(&peers(&[]), start + MESH_PEER_GRACE), []);
    }

    #[test]
    fn the_peers_end_with_the_mesh() {
        // The peers of a mesh that has stopped are none, so a watch that looks at them stops too.
        // Catches a handle that keeps the stopped mesh's registry alive and answers its last peers.
        let registry = std::sync::Arc::new(Mutex::new(MeshPeerRegistry {
            peers: Default::default(),
            history: Default::default(),
            history_limit: 4,
        }));
        registry
            .lock()
            .expect("the registry")
            .record("PlcB", true, 1);
        let handle = MeshPeers(std::sync::Arc::downgrade(&registry));
        assert_eq!(handle.current(), Some(peers(&["PlcB"])));
        drop(registry);
        assert_eq!(handle.current(), None);
    }
}
