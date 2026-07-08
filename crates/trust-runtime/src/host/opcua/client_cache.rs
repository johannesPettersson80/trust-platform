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
    Sample(OpcUaClientSample),
    ConnectionStatus {
        connected: bool,
        at_ms: u64,
        detail: String,
    },
    SessionClosed {
        at_ms: u64,
        detail: String,
    },
}

#[derive(Clone)]
pub struct OpcUaClientEventSink {
    sender: std::sync::mpsc::SyncSender<OpcUaClientWorkerEvent>,
}

impl OpcUaClientEventSink {
    fn new(sender: std::sync::mpsc::SyncSender<OpcUaClientWorkerEvent>) -> Self {
        Self { sender }
    }

    #[must_use]
    pub fn publish_sample(&self, sample: OpcUaClientSample) -> bool {
        self.sender
            .try_send(OpcUaClientWorkerEvent::Sample(sample))
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
                at_ms,
                detail: detail.into(),
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
    pending_writes: std::collections::BTreeMap<SmolStr, Value>,
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
                pending_writes: std::collections::BTreeMap::new(),
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
            pending_writes: guard.pending_writes.clone(),
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

    fn mark_stale(&self, detail: impl Into<String>) {
        self.mark_all_points(OpcUaClientConnectionState::Stale, None, None, detail);
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

    fn apply_sample(&self, sample: OpcUaClientSample) {
        let mut guard = self.inner.lock().unwrap_or_else(|err| err.into_inner());
        if sample.state == OpcUaClientConnectionState::Connected {
            guard.state = OpcUaClientConnectionState::Connected;
            guard.detail = "OPC UA client subscription update received.".to_string();
            guard.last_seen_ms = sample.last_seen_ms.or(guard.last_seen_ms);
            if let Some(value) = sample.value.clone() {
                guard.values.insert(sample.var.clone(), value);
            }
        } else if guard.state != OpcUaClientConnectionState::Faulted {
            guard.state = sample.state;
            guard.detail = sample.detail.clone();
        }
        let key = (sample.var.clone(), sample.node_id.clone());
        let status = guard
            .point_statuses
            .entry(key)
            .or_insert_with(|| OpcUaClientPointStatus {
                var: sample.var.clone(),
                node_id: sample.node_id.clone(),
                data_type: sample.data_type,
                access: sample.access,
                state: sample.state,
                last_seen_ms: None,
                value: None,
                detail: String::new(),
            });
        status.state = sample.state;
        status.last_seen_ms = sample.last_seen_ms.or(status.last_seen_ms);
        if sample.value.is_some() {
            status.value = sample.value;
        }
        status.detail = sample.detail;
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
        let status = guard
            .point_statuses
            .entry((point.var.clone(), point.node_id.clone()))
            .or_insert_with(|| OpcUaClientPointStatus {
                var: point.var.clone(),
                node_id: point.node_id.clone(),
                data_type: point.data_type,
                access: point.access,
                state,
                last_seen_ms: None,
                value: None,
                detail: String::new(),
            });
        status.state = state;
        status.last_seen_ms = last_seen_ms.or(status.last_seen_ms);
        if value.is_some() {
            status.value = value;
        }
        status.detail = detail;
    }

    fn queue_write(&self, point: &OpcUaClientPointConfig, value: Value) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .insert(point.var.clone(), value);
    }

    fn ack_write(&self, point: &OpcUaClientPointConfig) {
        self.inner
            .lock()
            .unwrap_or_else(|err| err.into_inner())
            .pending_writes
            .remove(point.var.as_str());
    }
}
