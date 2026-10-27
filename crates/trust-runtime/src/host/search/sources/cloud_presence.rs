//! The runtime cloud's presence of the runtimes that discovery saw (specification 11, "Runtime
//! cloud presence"). A runtime is read by the presence rule of the runtime cloud state, which
//! `runtime_cloud::projection` holds, and a stale or partitioned runtime carries its marks as
//! warnings. A runtime is read again at most once a second, because a web endpoint that does not
//! answer costs a connection attempt.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use smol_str::SmolStr;

use crate::discovery::DiscoveryEntry;
use crate::runtime_cloud::projection::{
    presence_observation_from_discovery, presence_record_from_observation,
    runtime_cloud_peer_appears_live, RuntimePresenceRecord, RUNTIME_CLOUD_DEFAULT_SITE,
    RUNTIME_CLOUD_PRESENCE,
};

/// How long a runtime's presence is kept before it is read again.
const PRESENCE_READ_PERIOD: Duration = Duration::from_secs(1);

/// The presence of the runtimes of one search, each read at most once per
/// [`PRESENCE_READ_PERIOD`].
#[derive(Debug, Default)]
pub(super) struct PresenceCache {
    reads: HashMap<SmolStr, (Instant, RuntimePresenceRecord)>,
}

impl PresenceCache {
    /// The presence of `entry` at `now`, on the clock of its announcement `now_ns`.
    pub(super) fn presence_of(
        &mut self,
        entry: &DiscoveryEntry,
        now: Instant,
        now_ns: u64,
    ) -> RuntimePresenceRecord {
        if let Some((read_at, record)) = self.reads.get(&entry.id)
            && now.saturating_duration_since(*read_at) < PRESENCE_READ_PERIOD
        {
            return record.clone();
        }
        let record = read_presence(entry, now_ns);
        self.reads.insert(entry.id.clone(), (now, record.clone()));
        record
    }

    /// Forgets the runtimes that discovery no longer lists.
    pub(super) fn retain_listed(&mut self, listed: &[DiscoveryEntry]) {
        self.reads
            .retain(|id, _| listed.iter().any(|entry| entry.id == *id));
    }
}

/// The presence of one runtime at `now_ns`: its observation under the runtime cloud's rule, and
/// the projection of that observation. The runtime is named by its runtime ID, as a find names it.
fn read_presence(entry: &DiscoveryEntry, now_ns: u64) -> RuntimePresenceRecord {
    let observation = presence_observation_from_discovery(
        RUNTIME_CLOUD_DEFAULT_SITE,
        entry.id.as_str(),
        entry.name.as_str(),
        entry.presence_sighting(),
        now_ns,
        || runtime_cloud_peer_appears_live(&entry.addresses, entry.web_port),
    );
    presence_record_from_observation(&observation, now_ns, RUNTIME_CLOUD_PRESENCE)
}

/// The warnings that mark a presence: `stale`, and `partitioned` as well when the runtime is
/// partitioned, named as the presence projection names them.
pub(super) fn presence_warnings(record: &RuntimePresenceRecord) -> Vec<String> {
    let mut warnings = Vec::new();
    if record.stale {
        warnings.push("stale".to_string());
    }
    if record.partitioned {
        warnings.push("partitioned".to_string());
    }
    warnings
}
