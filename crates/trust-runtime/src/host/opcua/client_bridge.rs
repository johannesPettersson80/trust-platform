#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientBinding {
    pub point: OpcUaClientPointConfig,
    pub reference: crate::value::ValueRef,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum OpcUaClientBridgeErrorKind {
    Validation,
    Transport,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpcUaClientBridgeError {
    message: String,
    kind: OpcUaClientBridgeErrorKind,
}

impl OpcUaClientBridgeError {
    fn validation(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: OpcUaClientBridgeErrorKind::Validation,
        }
    }

    fn transport(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            kind: OpcUaClientBridgeErrorKind::Transport,
        }
    }

    pub fn message(&self) -> &str {
        self.message.as_str()
    }

    fn is_transport(&self) -> bool {
        self.kind == OpcUaClientBridgeErrorKind::Transport
    }
}

impl std::fmt::Display for OpcUaClientBridgeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(self.message.as_str())
    }
}

impl std::error::Error for OpcUaClientBridgeError {}

#[derive(Clone)]
pub struct OpcUaClientBridge {
    bindings: Vec<OpcUaClientBinding>,
    shared: OpcUaSharedClientCache,
    last_queued_values: std::collections::BTreeMap<SmolStr, Value>,
}

impl OpcUaClientBridge {
    pub fn new(bindings: Vec<OpcUaClientBinding>) -> Result<Self, OpcUaClientBridgeError> {
        validate_opcua_client_bindings(&bindings)?;
        let points = bindings
            .iter()
            .map(|binding| binding.point.clone())
            .collect::<Vec<_>>();
        Ok(Self {
            bindings,
            shared: OpcUaSharedClientCache::new(points.as_slice()),
            last_queued_values: std::collections::BTreeMap::new(),
        })
    }

    pub fn with_transport<T: OpcUaClientTransport>(
        connection: OpcUaClientConnectionConfig,
        transport: T,
        bindings: Vec<OpcUaClientBinding>,
    ) -> Result<(Self, OpcUaClientWorker<T>), OpcUaClientBridgeError> {
        let bridge = Self::new(bindings)?;
        let worker = OpcUaClientWorker::new(connection, transport, bridge.shared.clone());
        Ok((bridge, worker))
    }

    pub fn apply_inputs(
        &mut self,
        storage: &mut crate::memory::VariableStorage,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.point.access.can_read())
        {
            let Some(status) = snapshot
                .point_statuses
                .iter()
                .find(|status| status.var == binding.point.var)
            else {
                self.shared.set_point_status(
                    &binding.point,
                    OpcUaClientConnectionState::Faulted,
                    Some(now_ms),
                    None,
                    format!("OPC UA cache has no status for '{}'", binding.point.var),
                );
                continue;
            };
            if status.state != OpcUaClientConnectionState::Connected {
                continue;
            }
            let Some(value) = snapshot.values.get(binding.point.var.as_str()).cloned() else {
                self.shared.set_point_status(
                    &binding.point,
                    OpcUaClientConnectionState::Faulted,
                    Some(now_ms),
                    None,
                    format!(
                        "OPC UA cache has connected status without a value for '{}'",
                        binding.point.var
                    ),
                );
                continue;
            };
            if !storage.write_by_ref(binding.reference.clone(), value.clone()) {
                self.shared.set_point_status(
                    &binding.point,
                    OpcUaClientConnectionState::Faulted,
                    Some(now_ms),
                    None,
                    format!("Failed to write OPC UA input '{}'", binding.point.var),
                );
                continue;
            }
            if binding.point.access.can_write() {
                self.last_queued_values
                    .insert(binding.point.var.clone(), value);
            }
        }
        Ok(())
    }

    pub fn capture_outputs(
        &mut self,
        storage: &mut crate::memory::VariableStorage,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.point.access.can_write())
        {
            let Some(value) = storage.read_by_ref(binding.reference.clone()).cloned() else {
                self.shared.set_point_status(
                    &binding.point,
                    OpcUaClientConnectionState::Faulted,
                    Some(now_ms),
                    None,
                    format!("Failed to read OPC UA output '{}'", binding.point.var),
                );
                continue;
            };
            if self
                .last_queued_values
                .get(binding.point.var.as_str())
                .is_some_and(|previous| previous == &value)
            {
                continue;
            }
            self.shared.queue_write(&binding.point, value.clone());
            self.last_queued_values
                .insert(binding.point.var.clone(), value);
        }
        Ok(())
    }

    pub fn state(&self) -> OpcUaClientConnectionState {
        self.shared.state()
    }

    pub fn snapshot(&self) -> OpcUaClientCacheSnapshot {
        self.shared.snapshot()
    }

    pub fn pending_write(&self, point_var: &str) -> Option<Value> {
        self.shared
            .snapshot()
            .pending_writes
            .get(point_var)
            .cloned()
    }
}

pub fn resolve_opcua_client_bindings(
    runtime: &crate::Runtime,
    connection: &OpcUaClientConnectionConfig,
) -> Result<Vec<OpcUaClientBinding>, RuntimeError> {
    let mut bindings = Vec::with_capacity(connection.points.len());
    for point in &connection.points {
        let storage_name = opcua_client_global_storage_name(point.var.as_str());
        let reference = runtime
            .storage()
            .ref_for_global(storage_name)
            .ok_or_else(|| {
                RuntimeError::InvalidConfig(
                    format!(
                        "OPC UA client connection '{}': global '{}' does not exist",
                        connection.name, point.var
                    )
                    .into(),
                )
            })?;
        validate_opcua_client_initial_value(runtime, connection, point, storage_name)?;
        bindings.push(OpcUaClientBinding {
            point: point.clone(),
            reference,
        });
    }
    Ok(bindings)
}

fn validate_opcua_client_bindings(
    bindings: &[OpcUaClientBinding],
) -> Result<(), OpcUaClientBridgeError> {
    let mut seen = std::collections::BTreeSet::new();
    for binding in bindings {
        if !seen.insert(binding.point.var.clone()) {
            return Err(OpcUaClientBridgeError::validation(format!(
                "OPC UA point '{}' is bound more than once",
                binding.point.var
            )));
        }
        if binding.point.access == OpcUaClientPointAccess::Write {
            return Err(OpcUaClientBridgeError::validation(format!(
                "OPC UA point '{}' must be readable before runtime can report live status",
                binding.point.var
            )));
        }
    }
    Ok(())
}

fn validate_opcua_client_initial_value(
    runtime: &crate::Runtime,
    connection: &OpcUaClientConnectionConfig,
    point: &OpcUaClientPointConfig,
    storage_name: &str,
) -> Result<(), RuntimeError> {
    let value = runtime.storage().get_global(storage_name).ok_or_else(|| {
        RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': global '{}' does not exist",
                connection.name, point.var
            )
            .into(),
        )
    })?;
    let Some(mapped) = map_iec_value(value) else {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': global '{}' has unsupported value {value:?}",
                connection.name, point.var
            )
            .into(),
        ));
    };
    if mapped.data_type != point.data_type {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': global '{}' is {}, but point expects {}",
                connection.name,
                point.var,
                mapped.data_type.as_config_value(),
                point.data_type.as_config_value()
            )
            .into(),
        ));
    }
    if point.access == OpcUaClientPointAccess::Write {
        return Err(RuntimeError::InvalidConfig(
            format!(
                "OPC UA client connection '{}': point '{}' must be readable before runtime can report live status",
                connection.name, point.var
            )
            .into(),
        ));
    }
    Ok(())
}

fn opcua_client_global_storage_name(point_var: &str) -> &str {
    point_var
        .split_once('.')
        .filter(|(prefix, name)| prefix.eq_ignore_ascii_case("global") && !name.is_empty())
        .map_or(point_var, |(_, name)| name)
}
