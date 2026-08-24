const DEFAULT_OPCUA_WORKER_TICK_INTERVAL: std::time::Duration =
    std::time::Duration::from_millis(20);
const DEFAULT_OPCUA_RECONNECT_BACKOFF_MS: u64 = 2_000;
const OPCUA_EVENT_CHANNEL_CAPACITY: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpcUaClientSessionInfo {
    pub requested_timeout_ms: u64,
    pub revised_timeout_ms: Option<u64>,
    pub recovery_detail: Option<&'static str>,
}

pub trait OpcUaClientTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError>;

    fn subscribe_read_points(
        &mut self,
        points: &[OpcUaClientPointConfig],
        sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError>;

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError>;

    fn recover_after_disconnect(
        &mut self,
        _connection: &OpcUaClientConnectionConfig,
        _read_points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<Option<OpcUaClientSessionInfo>, OpcUaClientBridgeError> {
        Ok(None)
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError>;
}

pub struct OpcUaClientWorker<T> {
    connection: OpcUaClientConnectionConfig,
    read_points: Vec<OpcUaClientPointConfig>,
    write_points: Vec<OpcUaClientPointConfig>,
    shared: OpcUaSharedClientCache,
    event_sender: std::sync::mpsc::SyncSender<OpcUaClientWorkerEvent>,
    events: std::sync::mpsc::Receiver<OpcUaClientWorkerEvent>,
    transport: T,
    reconnect_backoff_ms: u64,
    next_reconnect_after_ms: Option<u64>,
    connected_since_ms: Option<u64>,
    next_session_generation: u64,
    active_session_generation: Option<u64>,
    active_event_ms: Option<u64>,
}

impl<T: OpcUaClientTransport> OpcUaClientWorker<T> {
    fn new(
        connection: OpcUaClientConnectionConfig,
        transport: T,
        shared: OpcUaSharedClientCache,
    ) -> Self {
        let (sender, events) = std::sync::mpsc::sync_channel(OPCUA_EVENT_CHANNEL_CAPACITY);
        let read_points = connection
            .points
            .iter()
            .filter(|point| point.access.can_read())
            .cloned()
            .collect();
        let write_points = connection
            .points
            .iter()
            .filter(|point| point.access.can_write())
            .cloned()
            .collect();
        Self {
            connection,
            read_points,
            write_points,
            shared,
            event_sender: sender,
            events,
            transport,
            reconnect_backoff_ms: DEFAULT_OPCUA_RECONNECT_BACKOFF_MS,
            next_reconnect_after_ms: None,
            connected_since_ms: None,
            next_session_generation: 0,
            active_session_generation: None,
            active_event_ms: None,
        }
    }

    pub fn tick(&mut self, now_ms: u64) -> Result<(), OpcUaClientBridgeError> {
        self.drain_events(now_ms);
        match self.shared.state() {
            OpcUaClientConnectionState::Disabled | OpcUaClientConnectionState::Faulted => {
                return Ok(());
            }
            OpcUaClientConnectionState::Configured | OpcUaClientConnectionState::Connecting => {
                self.connect(now_ms)?;
            }
            OpcUaClientConnectionState::Reconnecting => {
                if self
                    .next_reconnect_after_ms
                    .is_some_and(|retry_at| now_ms < retry_at)
                {
                    return Ok(());
                }
                self.recover_or_reconnect(now_ms)?;
            }
            OpcUaClientConnectionState::Connected | OpcUaClientConnectionState::Stale => {}
        }

        if self.shared.state() == OpcUaClientConnectionState::Connected {
            self.publish_pending_writes(now_ms)?;
            self.mark_stale_if_due(now_ms);
        }
        Ok(())
    }

    pub fn connect(&mut self, now_ms: u64) -> Result<(), OpcUaClientBridgeError> {
        let (generation, sink) = self.next_session_candidate()?;
        self.shared.mark_connecting(
            now_ms,
            format!(
                "OPC UA client connection '{}' is connecting.",
                self.connection.name
            ),
        );
        let result = self
            .transport
            .connect(&self.connection, sink.clone())
            .and_then(|session| {
                self.transport
                    .subscribe_read_points(self.read_points.as_slice(), sink)?;
                Ok(session)
            });
        match result {
            Ok(session) => {
                self.active_session_generation = Some(generation);
                self.active_event_ms = Some(now_ms);
                self.connected_since_ms = Some(now_ms);
                self.next_reconnect_after_ms = None;
                self.shared.mark_connected(
                    now_ms,
                    session_detail(
                        self.read_points.len(),
                        session.requested_timeout_ms,
                        session.revised_timeout_ms,
                        session.recovery_detail,
                    ),
                );
                Ok(())
            }
            Err(error) => {
                self.active_session_generation = None;
                self.active_event_ms = None;
                self.handle_runtime_error(now_ms, &error);
                Err(error)
            }
        }
    }

    pub fn transport(&self) -> &T {
        &self.transport
    }

    pub fn transport_mut(&mut self) -> &mut T {
        &mut self.transport
    }

    pub fn mark_reconnecting(&mut self, now_ms: u64, detail: impl Into<String>) {
        let detail = detail.into();
        self.active_session_generation = None;
        self.active_event_ms = None;
        self.shared.mark_reconnecting(now_ms, detail.clone());
        self.next_reconnect_after_ms = Some(now_ms.saturating_add(self.reconnect_backoff_ms));
        self.connected_since_ms = None;
    }

    fn recover_or_reconnect(&mut self, now_ms: u64) -> Result<(), OpcUaClientBridgeError> {
        let (generation, sink) = self.next_session_candidate()?;
        match self.transport.recover_after_disconnect(
            &self.connection,
            self.read_points.as_slice(),
            sink,
        ) {
            Ok(Some(session)) => {
                self.active_session_generation = Some(generation);
                self.active_event_ms = Some(now_ms);
                self.connected_since_ms = Some(now_ms);
                self.next_reconnect_after_ms = None;
                self.shared.mark_connected(
                    now_ms,
                    session_detail(
                        self.read_points.len(),
                        session.requested_timeout_ms,
                        session.revised_timeout_ms,
                        session.recovery_detail,
                    ),
                );
                Ok(())
            }
            Ok(None) => {
                let _ = self.transport.disconnect();
                self.connect(now_ms)
            }
            Err(error) => {
                if error.is_transport() {
                    let _ = self.transport.disconnect();
                    self.connect(now_ms)
                } else {
                    self.handle_runtime_error(now_ms, &error);
                    Err(error)
                }
            }
        }
    }

    fn drain_events(&mut self, now_ms: u64) {
        while let Ok(event) = self.events.try_recv() {
            match event {
                OpcUaClientWorkerEvent::Sample { generation, sample } => {
                    if !self.event_is_current(generation, sample.last_seen_ms.unwrap_or(now_ms)) {
                        continue;
                    }
                    if let Err(detail) = self.shared.apply_sample(sample) {
                        self.active_session_generation = None;
                        self.active_event_ms = None;
                        self.shared.mark_faulted(now_ms, detail);
                    }
                }
                OpcUaClientWorkerEvent::ConnectionStatus {
                    generation,
                    connected,
                    at_ms,
                    detail,
                } => {
                    if !self.event_is_current(generation, at_ms) {
                        continue;
                    }
                    if connected {
                        self.connected_since_ms = Some(at_ms);
                        self.shared.mark_connected(at_ms, detail);
                    } else {
                        self.mark_reconnecting(at_ms, detail);
                    }
                }
                OpcUaClientWorkerEvent::SessionClosed {
                    generation,
                    at_ms,
                    detail,
                } => {
                    if !self.event_is_current(generation, at_ms) {
                        continue;
                    }
                    self.mark_reconnecting(at_ms, detail);
                }
            }
        }
        self.mark_stale_if_due(now_ms);
    }

    fn publish_pending_writes(&mut self, now_ms: u64) -> Result<(), OpcUaClientBridgeError> {
        let pending = self.shared.pending_write_batch();
        if pending.is_empty() {
            return Ok(());
        }
        let mut writes = Vec::new();
        for point in &self.write_points {
            if let Some(pending) = pending.get(point.var.as_str()) {
                writes.push((point.clone(), pending.value.clone(), pending.generation));
            }
        }
        if writes.is_empty() {
            return Ok(());
        }
        let transport_writes = writes
            .iter()
            .map(|(point, value, _)| (point.clone(), value.clone()))
            .collect::<Vec<_>>();
        if let Err(error) = self.transport.write_values(transport_writes.as_slice()) {
            if error.is_transport() {
                self.handle_runtime_error(now_ms, &error);
                return Err(error);
            }
            for (point, _, generation) in &writes {
                self.shared.complete_write(
                    point,
                    *generation,
                    OpcUaClientConnectionState::Faulted,
                    now_ms,
                    None,
                    format!("OPC UA client write rejected: {error}"),
                );
            }
            return Ok(());
        }
        for (point, value, generation) in writes {
            self.shared.complete_write(
                &point,
                generation,
                OpcUaClientConnectionState::Connected,
                now_ms,
                Some(value),
                "Live OPC UA client value written through persistent session.",
            );
        }
        Ok(())
    }

    fn mark_stale_if_due(&self, now_ms: u64) {
        let timeout_ms = self
            .connection
            .timeout_ms
            .max(self.connection.poll_interval_ms);
        let reference_ms = self
            .shared
            .last_seen_ms()
            .or(self.connected_since_ms)
            .unwrap_or(now_ms);
        if now_ms.saturating_sub(reference_ms) > timeout_ms
            && self.shared.state() == OpcUaClientConnectionState::Connected
        {
            self.shared.mark_stale(format!(
                "OPC UA client connection '{}' has no fresh subscription update within {} ms.",
                self.connection.name, timeout_ms
            ));
        }
    }

    fn handle_runtime_error(&mut self, now_ms: u64, error: &OpcUaClientBridgeError) {
        if error.is_transport() {
            self.mark_reconnecting(now_ms, error.to_string());
        } else {
            self.shared.mark_faulted(now_ms, error.to_string());
        }
    }

    fn next_session_candidate(
        &mut self,
    ) -> Result<(u64, OpcUaClientEventSink), OpcUaClientBridgeError> {
        self.next_session_generation =
            self.next_session_generation.checked_add(1).ok_or_else(|| {
                OpcUaClientBridgeError::validation("OPC UA client session generation exhausted")
            })?;
        let generation = self.next_session_generation;
        Ok((
            generation,
            OpcUaClientEventSink::new(self.event_sender.clone(), generation),
        ))
    }

    fn event_is_current(&mut self, generation: u64, at_ms: u64) -> bool {
        if self.active_session_generation != Some(generation)
            || self
                .active_event_ms
                .is_some_and(|accepted| at_ms < accepted)
        {
            return false;
        }
        self.active_event_ms = Some(
            self.active_event_ms
                .map_or(at_ms, |accepted| accepted.max(at_ms)),
        );
        true
    }
}

pub struct OpcUaClientWorkerThread {
    stop: std::sync::Arc<std::sync::atomic::AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

impl OpcUaClientWorkerThread {
    pub fn shutdown(mut self) -> Result<(), OpcUaClientBridgeError> {
        self.request_stop();
        self.join_worker()
    }

    fn request_stop(&self) {
        self.stop.store(true, std::sync::atomic::Ordering::SeqCst);
        if let Some(join) = &self.join {
            join.thread().unpark();
        }
    }

    fn join_worker(&mut self) -> Result<(), OpcUaClientBridgeError> {
        let Some(join) = self.join.take() else {
            return Ok(());
        };
        join.join()
            .map_err(|_| OpcUaClientBridgeError::validation("OPC UA client worker thread panicked"))
    }
}

impl Drop for OpcUaClientWorkerThread {
    fn drop(&mut self) {
        self.request_stop();
        let _ = self.join_worker();
    }
}

impl<T: OpcUaClientTransport + Send + 'static> OpcUaClientWorker<T> {
    pub fn spawn(
        self,
        tick_interval: std::time::Duration,
    ) -> Result<OpcUaClientWorkerThread, OpcUaClientBridgeError> {
        let stop = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let stop_ref = std::sync::Arc::clone(&stop);
        let interval = if tick_interval.is_zero() {
            DEFAULT_OPCUA_WORKER_TICK_INTERVAL
        } else {
            tick_interval
        };
        let join = std::thread::Builder::new()
            .name("trust-opcua-client-worker".to_string())
            .spawn(move || {
                let mut worker = self;
                while !stop_ref.load(std::sync::atomic::Ordering::SeqCst) {
                    let _ = worker.tick(opcua_client_now_ms());
                    std::thread::park_timeout(interval);
                }
                let _ = worker.transport.disconnect();
                worker
                    .shared
                    .mark_shutdown("OPC UA client worker stopped; readable values are not live.");
            })
            .map_err(|err| {
                OpcUaClientBridgeError::transport(format!(
                    "failed to spawn OPC UA client worker thread: {err}"
                ))
            })?;
        Ok(OpcUaClientWorkerThread {
            stop,
            join: Some(join),
        })
    }
}

#[cfg(feature = "opcua-wire")]
pub struct OpcUaWireClientTransport {
    session: Option<std::sync::Arc<::opcua::sync::RwLock<::opcua::client::prelude::Session>>>,
    runner: Option<std::thread::JoinHandle<()>>,
}

#[cfg(feature = "opcua-wire")]
impl OpcUaWireClientTransport {
    #[must_use]
    pub fn new() -> Self {
        Self {
            session: None,
            runner: None,
        }
    }
}

#[cfg(feature = "opcua-wire")]
impl Default for OpcUaWireClientTransport {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(feature = "opcua-wire")]
impl OpcUaClientTransport for OpcUaWireClientTransport {
    fn connect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        sink: OpcUaClientEventSink,
    ) -> Result<OpcUaClientSessionInfo, OpcUaClientBridgeError> {
        let _ = self.disconnect();
        let session = connect_opcua_client_session(
            connection.endpoint_url.as_str(),
            connection.security,
            &connection.auth,
            connection.trust_server_certificate,
            "truST OPC UA runtime client",
            "urn:trust:runtime:opcua:persistent-client",
        )
        .map_err(|err| OpcUaClientBridgeError::transport(err.to_string()))?;
        {
            let mut session_guard = session.write();
            let status_sink = sink.clone();
            session_guard.set_connection_status_callback(
                ::opcua::client::prelude::ConnectionStatusCallback::new(move |connected| {
                    let detail = if connected {
                        "OPC UA client session connected."
                    } else {
                        "OPC UA client session disconnected."
                    };
                    let _ = status_sink.publish_connection_status(
                        connected,
                        opcua_client_now_ms(),
                        detail,
                    );
                }),
            );
            let closed_sink = sink;
            session_guard.set_session_closed_callback(
                ::opcua::client::prelude::SessionClosedCallback::new(move |status| {
                    let _ = closed_sink.publish_session_closed(
                        opcua_client_now_ms(),
                        format!("OPC UA client session closed: {status}"),
                    );
                }),
            );
        }
        let runner_session = session.clone();
        let runner = std::thread::Builder::new()
            .name("trust-opcua-session-runner".to_string())
            .spawn(move || {
                ::opcua::client::prelude::Session::run(runner_session);
            })
            .map_err(|err| {
                OpcUaClientBridgeError::transport(format!(
                    "failed to spawn OPC UA session runner: {err}"
                ))
            })?;
        self.session = Some(session);
        self.runner = Some(runner);
        // opcua 0.12 logs the revised session timeout internally when the
        // session is created, but does not expose it through the public
        // Session API. The worker therefore records None and uses the existing
        // configured timeout for truST stale detection.
        Ok(OpcUaClientSessionInfo {
            requested_timeout_ms: connection.timeout_ms,
            revised_timeout_ms: None,
            recovery_detail: None,
        })
    }

    fn subscribe_read_points(
        &mut self,
        points: &[OpcUaClientPointConfig],
        sink: OpcUaClientEventSink,
    ) -> Result<(), OpcUaClientBridgeError> {
        if points.is_empty() {
            return Ok(());
        }
        let session = self.session.as_ref().ok_or_else(|| {
            OpcUaClientBridgeError::transport("OPC UA client session is not connected")
        })?;
        let point_map = points
            .iter()
            .map(|point| {
                parse_node_id(point.node_id.as_str())
                    .map(|node_id| (node_id, point.clone()))
                    .map_err(|err| OpcUaClientBridgeError::validation(err.to_string()))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let callback_points = point_map.clone();
        let subscription_id = {
            let session = session.read();
            ::opcua::client::prelude::SubscriptionService::create_subscription(
                &*session,
                250.0,
                10,
                30,
                0,
                0,
                true,
                ::opcua::client::prelude::DataChangeCallback::new(move |items| {
                    for item in items {
                        let Some((_, point)) = callback_points
                            .iter()
                            .find(|(node_id, _)| node_id == &item.item_to_monitor().node_id)
                        else {
                            continue;
                        };
                        let sample = sample_from_data_value(point, item.last_value());
                        let _ = sink.publish_sample(sample);
                    }
                }),
            )
            .map_err(|err| OpcUaClientBridgeError::transport(err.to_string()))?
        };
        let items = point_map
            .into_iter()
            .map(
                |(node_id, _)| ::opcua::client::prelude::MonitoredItemCreateRequest {
                    item_to_monitor: ::opcua::client::prelude::ReadValueId {
                        node_id,
                        attribute_id: ::opcua::client::prelude::AttributeId::Value as u32,
                        index_range: ::opcua::client::prelude::UAString::null(),
                        data_encoding: ::opcua::client::prelude::QualifiedName::null(),
                    },
                    monitoring_mode: ::opcua::client::prelude::MonitoringMode::Reporting,
                    requested_parameters: ::opcua::client::prelude::MonitoringParameters {
                        client_handle: 0,
                        sampling_interval: 0.0,
                        filter: ::opcua::client::prelude::ExtensionObject::null(),
                        queue_size: 1,
                        discard_oldest: true,
                    },
                },
            )
            .collect::<Vec<_>>();
        {
            let session = session.read();
            ::opcua::client::prelude::MonitoredItemService::create_monitored_items(
                &*session,
                subscription_id,
                ::opcua::client::prelude::TimestampsToReturn::Both,
                items.as_slice(),
            )
            .map_err(|err| OpcUaClientBridgeError::transport(err.to_string()))?;
        }
        Ok(())
    }

    fn write_values(
        &mut self,
        values: &[(OpcUaClientPointConfig, Value)],
    ) -> Result<(), OpcUaClientBridgeError> {
        if values.is_empty() {
            return Ok(());
        }
        let session = self.session.as_ref().ok_or_else(|| {
            OpcUaClientBridgeError::transport("OPC UA client session is not connected")
        })?;
        let write_values = values
            .iter()
            .map(|(point, value)| {
                let mapped = map_iec_value(value).ok_or_else(|| {
                    OpcUaClientBridgeError::validation(format!(
                        "OPC UA point '{}' has unsupported value {value:?}",
                        point.var
                    ))
                })?;
                if mapped.data_type != point.data_type {
                    return Err(OpcUaClientBridgeError::validation(format!(
                        "OPC UA point '{}' expected {}, got {}",
                        point.var,
                        point.data_type.as_config_value(),
                        mapped.data_type.as_config_value()
                    )));
                }
                Ok(::opcua::client::prelude::WriteValue {
                    node_id: parse_node_id(point.node_id.as_str())
                        .map_err(|err| OpcUaClientBridgeError::validation(err.to_string()))?,
                    attribute_id: ::opcua::client::prelude::AttributeId::Value as u32,
                    index_range: ::opcua::client::prelude::UAString::null(),
                    value: ::opcua::client::prelude::DataValue {
                        value: Some(to_wire_variant(&mapped.value)),
                        status: Some(::opcua::client::prelude::StatusCode::Good),
                        source_timestamp: Some(::opcua::client::prelude::DateTime::now()),
                        ..Default::default()
                    },
                })
            })
            .collect::<Result<Vec<_>, OpcUaClientBridgeError>>()?;
        let statuses = {
            let session = session.read();
            session
                .write(write_values.as_slice())
                .map_err(|err| OpcUaClientBridgeError::transport(err.to_string()))?
        };
        validate_opcua_write_statuses(values, statuses.as_slice())
    }

    fn recover_after_disconnect(
        &mut self,
        connection: &OpcUaClientConnectionConfig,
        _read_points: &[OpcUaClientPointConfig],
        _sink: OpcUaClientEventSink,
    ) -> Result<Option<OpcUaClientSessionInfo>, OpcUaClientBridgeError> {
        let Some(session) = self.session.as_ref() else {
            return Ok(None);
        };
        let recovered = {
            let mut session = session.write();
            session.reconnect_and_activate()
        };
        match recovered {
            Ok(()) => Ok(Some(OpcUaClientSessionInfo {
                requested_timeout_ms: connection.timeout_ms,
                revised_timeout_ms: None,
                recovery_detail: Some(
                    "reconnect_and_activate transferred subscriptions with send_initial_values=true or recreated them; explicit RepublishRequest is unavailable because opcua 0.12 keeps publish sequence state private",
                ),
            })),
            Err(status) => Err(OpcUaClientBridgeError::transport(format!(
                "OPC UA reconnect_and_activate failed: {status}"
            ))),
        }
    }

    fn disconnect(&mut self) -> Result<(), OpcUaClientBridgeError> {
        if let Some(session) = self.session.take() {
            session.read().disconnect();
        }
        if let Some(runner) = self.runner.take() {
            runner
                .join()
                .map_err(|_| OpcUaClientBridgeError::transport("OPC UA session runner panicked"))?;
        }
        Ok(())
    }
}

#[cfg(feature = "opcua-wire")]
impl Drop for OpcUaWireClientTransport {
    fn drop(&mut self) {
        let _ = self.disconnect();
    }
}

#[cfg(feature = "opcua-wire")]
fn sample_from_data_value(
    point: &OpcUaClientPointConfig,
    data_value: &::opcua::client::prelude::DataValue,
) -> OpcUaClientSample {
    let now_ms = opcua_client_now_ms();
    let status = data_value
        .status
        .unwrap_or(::opcua::client::prelude::StatusCode::Good);
    if !status.is_good() {
        return OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: None,
            state: OpcUaClientConnectionState::Stale,
            last_seen_ms: None,
            detail: format!(
                "OPC UA node '{}' subscription returned {status}",
                point.node_id
            ),
        };
    }
    let Some(variant) = data_value.value.as_ref() else {
        return OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: None,
            state: OpcUaClientConnectionState::Stale,
            last_seen_ms: None,
            detail: format!(
                "OPC UA node '{}' subscription returned no value",
                point.node_id
            ),
        };
    };
    let Some(variant) = from_wire_variant(variant) else {
        return OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: None,
            state: OpcUaClientConnectionState::Faulted,
            last_seen_ms: Some(now_ms),
            detail: format!(
                "OPC UA node '{}' returned unsupported value {variant:?}",
                point.node_id
            ),
        };
    };
    match value_from_opcua_variant(point.data_type, variant) {
        Ok(value) => OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: Some(value),
            state: OpcUaClientConnectionState::Connected,
            last_seen_ms: Some(now_ms),
            detail: "Live OPC UA client subscription value received.".to_string(),
        },
        Err(error) => OpcUaClientSample {
            var: point.var.clone(),
            node_id: point.node_id.clone(),
            data_type: point.data_type,
            access: point.access,
            value: None,
            state: OpcUaClientConnectionState::Faulted,
            last_seen_ms: Some(now_ms),
            detail: error.to_string(),
        },
    }
}

#[cfg(feature = "opcua-wire")]
fn validate_opcua_write_statuses(
    values: &[(OpcUaClientPointConfig, Value)],
    statuses: &[::opcua::client::prelude::StatusCode],
) -> Result<(), OpcUaClientBridgeError> {
    if statuses.len() != values.len() {
        return Err(OpcUaClientBridgeError::validation(format!(
            "OPC UA write returned {} status result(s) for {} requested write(s)",
            statuses.len(),
            values.len()
        )));
    }
    for ((point, _), status) in values.iter().zip(statuses) {
        if !status.is_good() {
            return Err(OpcUaClientBridgeError::validation(format!(
                "OPC UA node '{}' write returned {status}",
                point.node_id
            )));
        }
    }
    Ok(())
}

fn session_detail(
    monitored_items: usize,
    requested_timeout_ms: u64,
    revised_timeout_ms: Option<u64>,
    recovery_detail: Option<&'static str>,
) -> String {
    let mut detail = match revised_timeout_ms {
        Some(timeout) => format!(
            "OPC UA client connected; {monitored_items} monitored item(s), requested stale timeout {requested_timeout_ms} ms, revised session timeout {timeout} ms."
        ),
        None => format!(
            "OPC UA client connected; {monitored_items} monitored item(s), session keep-alive active; revised session timeout is not exposed by opcua 0.12, using configured stale timeout {requested_timeout_ms} ms."
        ),
    };
    if let Some(recovery_detail) = recovery_detail {
        detail.push(' ');
        detail.push_str(recovery_detail);
        detail.push('.');
    }
    detail
}

pub fn opcua_client_now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}
