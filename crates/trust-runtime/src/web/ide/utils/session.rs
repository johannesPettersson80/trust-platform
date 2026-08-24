use super::*;
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use rand::TryRng;

pub(in crate::web::ide) fn prune_expired(state: &mut IdeStateInner, now: u64) {
    let expired = state
        .sessions
        .iter()
        .filter_map(|(token, session)| {
            if session.expires_at <= now {
                Some(token.clone())
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    if expired.is_empty() {
        return;
    }

    for token in &expired {
        remove_session(state, token.as_str());
    }
}

pub(in crate::web::ide) fn remove_session(state: &mut IdeStateInner, token: &str) {
    state.sessions.remove(token);
    state.frontend_telemetry_by_session.remove(token);
    state.analysis_cache.remove(token);
    for doc in state.documents.values_mut() {
        doc.opened_by.remove(token);
    }
}

pub(in crate::web::ide) fn generate_token() -> String {
    let mut bytes = [0_u8; 32];
    let mut rng = rand::rngs::SysRng;
    let _ = rng.try_fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

pub(in crate::web::ide) fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
#[path = "session/contract_tests.rs"]
mod contract_tests;
