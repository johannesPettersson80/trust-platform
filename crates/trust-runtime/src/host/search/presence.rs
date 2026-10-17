//! Presence within one search (specification 11, "Presence"): a result seen again is not listed
//! twice; it moves its `last_seen` time forward and keeps the stronger confidence. Pure.

use crate::connectors::DiscoveryConfidence;

use super::model::ProtocolFind;

/// Folds `again`, the same result seen again, into `known`; answers whether `known` changed. A
/// result at least as strong as the known one replaces it, so its parameters and warnings stay
/// those of the evidence it names; a weaker one only moves `last_seen` forward.
pub(crate) fn reobserve(known: &mut ProtocolFind, again: ProtocolFind) -> bool {
    let last_seen_ms = known.last_seen_ms.max(again.last_seen_ms);
    let updated = if rank(again.confidence) >= rank(known.confidence) {
        ProtocolFind {
            last_seen_ms,
            ..again
        }
    } else {
        ProtocolFind {
            last_seen_ms,
            ..known.clone()
        }
    };
    let changed = *known != updated;
    *known = updated;
    changed
}

/// The order of the confidence vocabulary, from no evidence to a protocol exchange.
fn rank(confidence: DiscoveryConfidence) -> u8 {
    match confidence {
        DiscoveryConfidence::Unavailable => 0,
        DiscoveryConfidence::PortReachable => 1,
        DiscoveryConfidence::Likely => 2,
        DiscoveryConfidence::Confirmed => 3,
    }
}
