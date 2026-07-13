//! Read-only ADS live-values DAP request/event handling.

use serde_json::Value;

use crate::protocol::{AdsStateEventBody, Request};

use super::{DebugAdapter, DispatchOutcome};

impl DebugAdapter {
    pub(super) fn handle_ads_state(&mut self, request: Request<Value>) -> DispatchOutcome {
        let body = if let Some(remote) = self.remote_session.as_mut() {
            match remote.ads_live_values() {
                Ok(body) => body,
                Err(error) => {
                    return DispatchOutcome {
                        responses: vec![self.error_response(&request, &error)],
                        ..DispatchOutcome::default()
                    };
                }
            }
        } else {
            let runtime = self.session.runtime_handle();
            let snapshot = match runtime.try_lock() {
                Ok(runtime) => runtime.ads_live_values_snapshot(),
                Err(std::sync::TryLockError::Poisoned(poisoned)) => {
                    poisoned.into_inner().ads_live_values_snapshot()
                }
                Err(std::sync::TryLockError::WouldBlock) => {
                    let cached = self
                        .last_ads_state
                        .lock()
                        .ok()
                        .and_then(|cache| cache.clone());
                    let Some(cached) = cached else {
                        return DispatchOutcome {
                            responses: vec![self.error_response(&request, "runtime busy")],
                            ..DispatchOutcome::default()
                        };
                    };
                    cached
                }
            };
            snapshot
        };

        if let Ok(mut cache) = self.last_ads_state.lock() {
            *cache = Some(body.clone());
        }
        DispatchOutcome {
            responses: vec![self.ok_response(&request, Some(body.clone()))],
            events: vec![self.event("stAdsState", Some(body))],
            ..DispatchOutcome::default()
        }
    }

    pub(super) fn capture_ads_state_from_runtime(&self) -> Option<AdsStateEventBody> {
        let runtime = self.session.runtime_handle();
        let runtime = runtime.try_lock().ok()?;
        let body = runtime.ads_live_values_snapshot();
        if let Ok(mut cache) = self.last_ads_state.lock() {
            *cache = Some(body.clone());
        }
        Some(body)
    }

    pub(super) fn emit_ads_state_event_from_runtime(&self, events: &mut Vec<Value>) {
        if let Some(body) = self.capture_ads_state_from_runtime() {
            events.push(self.event("stAdsState", Some(body)));
        }
    }
}
