#[derive(Debug, Clone, PartialEq)]
pub struct OpcUaClientSample {
    pub var: SmolStr,
    pub node_id: String,
    pub data_type: OpcUaDataType,
    pub access: OpcUaClientPointAccess,
    pub value: Option<Value>,
    pub state: OpcUaClientConnectionState,
    pub last_seen_ms: Option<u64>,
    pub detail: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum OpcUaClientWorkerEvent {
    Sample {
        generation: u64,
        sample: OpcUaClientSample,
    },
    ConnectionStatus {
        generation: u64,
        connected: bool,
        at_ms: u64,
        detail: String,
    },
    SessionClosed {
        generation: u64,
        at_ms: u64,
        detail: String,
    },
    /// A notification of the subscription's heartbeat, the server's current time: evidence that
    /// the subscription is alive, though no point's value changed (specification 11).
    Heartbeat { generation: u64, at_ms: u64 },
}

#[derive(Clone)]
pub struct OpcUaClientEventSink {
    sender: std::sync::mpsc::SyncSender<OpcUaClientWorkerEvent>,
    generation: u64,
}

impl OpcUaClientEventSink {
    fn new(sender: std::sync::mpsc::SyncSender<OpcUaClientWorkerEvent>, generation: u64) -> Self {
        Self { sender, generation }
    }

    #[must_use]
    pub fn publish_sample(&self, sample: OpcUaClientSample) -> bool {
        self.sender
            .try_send(OpcUaClientWorkerEvent::Sample {
                generation: self.generation,
                sample,
            })
            .is_ok()
    }

    #[must_use]
    pub fn publish_connection_status(
        &self,
        connected: bool,
        at_ms: u64,
        detail: impl Into<String>,
    ) -> bool {
        self.sender
            .try_send(OpcUaClientWorkerEvent::ConnectionStatus {
                generation: self.generation,
                connected,
                at_ms,
                detail: detail.into(),
            })
            .is_ok()
    }

    #[must_use]
    pub fn publish_session_closed(&self, at_ms: u64, detail: impl Into<String>) -> bool {
        self.sender
            .try_send(OpcUaClientWorkerEvent::SessionClosed {
                generation: self.generation,
                at_ms,
                detail: detail.into(),
            })
            .is_ok()
    }

    /// Publishes a notification of the subscription's heartbeat at `at_ms`.
    #[must_use]
    pub fn publish_heartbeat(&self, at_ms: u64) -> bool {
        self.sender
            .try_send(OpcUaClientWorkerEvent::Heartbeat {
                generation: self.generation,
                at_ms,
            })
            .is_ok()
    }
}

#[derive(Clone)]
pub struct OpcUaSharedClientCache {
    inner: std::sync::Arc<std::sync::Mutex<OpcUaClientCacheState>>,
}

#[derive(Debug, Clone)]
struct OpcUaClientCacheState {
    state: OpcUaClientConnectionState,
    detail: String,
    last_seen_ms: Option<u64>,
    values: std::collections::BTreeMap<SmolStr, Value>,
    point_statuses: std::collections::BTreeMap<(SmolStr, String), OpcUaClientPointStatus>,
    /// The readable points that silence made stale, which the next notification makes live again.
    silenced: std::collections::BTreeSet<(SmolStr, String)>,
    pending_writes: std::collections::BTreeMap<SmolStr, OpcUaPendingWrite>,
    next_write_generation: u64,
}

#[derive(Debug, Clone)]
struct OpcUaPendingWrite {
    generation: u64,
    value: Value,
}

#[derive(Debug, Clone)]
pub struct OpcUaClientCacheSnapshot {
    pub state: OpcUaClientConnectionState,
    pub detail: String,
    pub last_seen_ms: Option<u64>,
    pub values: std::collections::BTreeMap<SmolStr, Value>,
    pub point_statuses: Vec<OpcUaClientPointStatus>,
    pub pending_writes: std::collections::BTreeMap<SmolStr, Value>,
}

impl OpcUaSharedClientCache {
    fn new(points: &[OpcUaClientPointConfig]) -> Self {
        let point_statuses = points
            .iter()
            .map(|point| {
                (
                    (point.var.clone(), point.node_id.clone()),
                    OpcUaClientPointStatus {
                        var: point.var.clone(),
                        node_id: point.node_id.clone(),
                        data_type: point.data_type,
                        access: point.access,
                        state: OpcUaClientConnectionState::Configured,
                        last_seen_ms: None,
                        value: None,
                        detail: "Configured; no live subscription update has completed yet."
                            .to_string(),
                    },
                )
            })
            .collect();
        Self {
            inner: std::sync::Arc::new(std::sync::Mutex::new(OpcUaClientCacheState {
                state: OpcUaClientConnectionState::Configured,
                detail: "Configured; persistent client worker has not connected yet.".to_string(),
                last_seen_ms: None,
                values: std::collections::BTreeMap::new(),
                point_statuses,
                silenced: std::collections::BTreeSet::new(),
                pending_writes: std::collections::BTreeMap::new(),
                next_write_generation: 0,
            })),
        }
    }

    pub fn snapshot(&self) -> OpcUaClientCacheSnapshot {
        let guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        OpcUaClientCacheSnapshot {
            state: guard.state,
            detail: guard.detail.clone(),
            last_seen_ms: guard.last_seen_ms,
            values: guard.values.clone(),
            point_statuses: guard.point_statuses.values().cloned().collect(),
            pending_writes: guard
                .pending_writes
                .iter()
                .map(|(var, pending)| (var.clone(), pending.value.clone()))
                .collect(),
        }
    }

    pub fn state(&self) -> OpcUaClientConnectionState {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .state
    }

    pub fn last_seen_ms(&self) -> Option<u64> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .last_seen_ms
    }

    fn mark_connecting(&self, now_ms: u64, detail: impl Into<String>) {
        self.mark_all_points(
            OpcUaClientConnectionState::Connecting,
            Some(now_ms),
            None,
            detail,
        );
    }

    fn mark_connected(&self, now_ms: u64, detail: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        guard.state = OpcUaClientConnectionState::Connected;
        guard.detail = detail.into();
        guard.last_seen_ms = Some(now_ms);
    }

    fn mark_reconnecting(&self, now_ms: u64, detail: impl Into<String>) {
        self.mark_all_points(
            OpcUaClientConnectionState::Reconnecting,
            Some(now_ms),
            None,
            detail,
        );
    }

    /// Marks the connection stale after silence, no notification for its stale bound
    /// (specification 11). Only the readable points that were live become stale, and they are
    /// remembered, so that the next notification makes them live again; a point's fault, its
    /// server's status or a point without a first value stays as it is.
    fn mark_silent(&self, detail: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let detail = detail.into();
        guard.state = OpcUaClientConnectionState::Stale;
        guard.detail = detail.clone();
        let mut silenced = Vec::new();
        for (key, status) in &mut guard.point_statuses {
            if status.access.can_read() && status.state == OpcUaClientConnectionState::Connected {
                status.state = OpcUaClientConnectionState::Stale;
                status.detail = detail.clone();
                silenced.push(key.clone());
            }
        }
        guard.silenced.extend(silenced);
    }

    /// A notification of the subscription's heartbeat at `at_ms` (specification 11): the
    /// subscription is alive, so the connection is fresh, and live again after silence.
    fn apply_heartbeat(&self, at_ms: u64) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        if revive(&mut guard, Some(at_ms)) {
            guard.detail = "OPC UA client subscription heartbeat received after silence.".to_string();
        }
    }

    fn mark_shutdown(&self, detail: impl Into<String>) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let detail = detail.into();
        guard.state = OpcUaClientConnectionState::Stale;
        guard.detail = detail.clone();
        guard.silenced.clear();
        for status in guard.point_statuses.values_mut() {
            if status.access.can_read() && status.state == OpcUaClientConnectionState::Connected {
                status.state = OpcUaClientConnectionState::Stale;
                status.detail = detail.clone();
            }
        }
    }

    fn mark_faulted(&self, now_ms: u64, detail: impl Into<String>) {
        self.mark_all_points(
            OpcUaClientConnectionState::Faulted,
            Some(now_ms),
            None,
            detail,
        );
    }

    fn mark_all_points(
        &self,
        state: OpcUaClientConnectionState,
        transition_ms: Option<u64>,
        value: Option<Value>,
        detail: impl Into<String>,
    ) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let detail = detail.into();
        guard.state = state;
        guard.detail = detail.clone();
        guard.silenced.clear();
        if state == OpcUaClientConnectionState::Connected {
            guard.last_seen_ms = transition_ms.or(guard.last_seen_ms);
        }
        for status in guard.point_statuses.values_mut() {
            status.state = state;
            status.last_seen_ms = if state == OpcUaClientConnectionState::Connected {
                transition_ms.or(status.last_seen_ms)
            } else {
                status.last_seen_ms
            };
            if value.is_some() {
                status.value = value.clone();
            }
            status.detail = detail.clone();
        }
    }

    fn apply_sample(&self, sample: OpcUaClientSample) -> Result<(), String> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let key = (sample.var.clone(), sample.node_id.clone());
        let Some(configured) = guard.point_statuses.get(&key) else {
            return Err(format!(
                "OPC UA subscription sample identity is not configured: var='{}', node='{}'",
                sample.var, sample.node_id
            ));
        };
        if configured.data_type != sample.data_type || configured.access != sample.access {
            return Err(format!(
                "OPC UA subscription sample identity disagrees with configured point '{}': node='{}', type='{}', access={:?}",
                sample.var,
                sample.node_id,
                sample.data_type.as_config_value(),
                sample.access
            ));
        }
        // A live sample proves the subscription alive; a sample that is not live changes only its
        // point, which the connection reports among its degraded points (specification 23).
        if sample.state == OpcUaClientConnectionState::Connected {
            revive(&mut guard, sample.last_seen_ms);
            guard.detail = "OPC UA client subscription update received.".to_string();
            if let Some(value) = sample.value.clone() {
                guard.values.insert(sample.var.clone(), value);
            }
        }
        guard.silenced.remove(&key);
        let status = guard
            .point_statuses
            .get_mut(&key)
            .expect("configured OPC UA point status disappeared while cache was locked");
        status.state = sample.state;
        status.last_seen_ms = sample.last_seen_ms.or(status.last_seen_ms);
        if sample.value.is_some() {
            status.value = sample.value;
        }
        status.detail = sample.detail;
        Ok(())
    }

    fn set_point_status(
        &self,
        point: &OpcUaClientPointConfig,
        state: OpcUaClientConnectionState,
        last_seen_ms: Option<u64>,
        value: Option<Value>,
        detail: impl Into<String>,
    ) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let detail = detail.into();
        let status = point_status_mut(&mut guard, point, state);
        status.state = state;
        status.last_seen_ms = last_seen_ms.or(status.last_seen_ms);
        if value.is_some() {
            status.value = value;
        }
        status.detail = detail;
    }

    fn pending_write_batch(&self) -> std::collections::BTreeMap<SmolStr, OpcUaPendingWrite> {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .clone()
    }

    fn queue_write(
        &self,
        point: &OpcUaClientPointConfig,
        value: Value,
    ) -> Result<u64, OpcUaClientBridgeError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let generation = next_write_generation(&mut guard)?;
        guard
            .pending_writes
            .insert(point.var.clone(), OpcUaPendingWrite { generation, value });
        Ok(generation)
    }

    fn reject_output(
        &self,
        point: &OpcUaClientPointConfig,
        now_ms: u64,
        detail: impl Into<String>,
    ) -> Result<(), OpcUaClientBridgeError> {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        let _ = next_write_generation(&mut guard)?;
        guard.pending_writes.remove(point.var.as_str());
        let status = point_status_mut(&mut guard, point, OpcUaClientConnectionState::Faulted);
        status.state = OpcUaClientConnectionState::Faulted;
        status.last_seen_ms = Some(now_ms).or(status.last_seen_ms);
        status.detail = detail.into();
        Ok(())
    }

    fn complete_write(
        &self,
        point: &OpcUaClientPointConfig,
        generation: u64,
        state: OpcUaClientConnectionState,
        now_ms: u64,
        value: Option<Value>,
        detail: impl Into<String>,
    ) -> bool {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        if guard
            .pending_writes
            .get(point.var.as_str())
            .is_none_or(|pending| pending.generation != generation)
        {
            return false;
        }
        guard.pending_writes.remove(point.var.as_str());
        let status = point_status_mut(&mut guard, point, state);
        status.state = state;
        status.last_seen_ms = Some(now_ms).or(status.last_seen_ms);
        if value.is_some() {
            status.value = value;
        }
        status.detail = detail.into();
        true
    }
}

/// A live notification at `at_ms`, a point's sample or the heartbeat, proves the subscription alive
/// (specification 11): the connection is fresh and connected, and the points that silence made
/// stale are live again with their last values. Whether the connection was stale.
fn revive(state: &mut OpcUaClientCacheState, at_ms: Option<u64>) -> bool {
    if let Some(at_ms) = at_ms {
        state.last_seen_ms = Some(state.last_seen_ms.map_or(at_ms, |seen| seen.max(at_ms)));
    }
    let was_stale = state.state == OpcUaClientConnectionState::Stale;
    state.state = OpcUaClientConnectionState::Connected;
    for key in std::mem::take(&mut state.silenced) {
        if let Some(status) = state.point_statuses.get_mut(&key)
            && status.state == OpcUaClientConnectionState::Stale
        {
            status.state = OpcUaClientConnectionState::Connected;
            status.detail =
                "The subscription's notifications resumed; the last value is live.".to_string();
        }
    }
    was_stale
}

fn next_write_generation(state: &mut OpcUaClientCacheState) -> Result<u64, OpcUaClientBridgeError> {
    state.next_write_generation = state.next_write_generation.checked_add(1).ok_or_else(|| {
        OpcUaClientBridgeError::validation("OPC UA pending-write generation exhausted")
    })?;
    Ok(state.next_write_generation)
}

fn point_status_mut<'a>(
    state: &'a mut OpcUaClientCacheState,
    point: &OpcUaClientPointConfig,
    default_state: OpcUaClientConnectionState,
) -> &'a mut OpcUaClientPointStatus {
    state
        .point_statuses
        .entry((point.var.clone(), point.node_id.clone()))
        .or_insert_with(|| OpcUaClientPointStatus {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            state: default_state,
            last_seen_ms: None,
            value: None,
            detail: String::new(),
        })
}
