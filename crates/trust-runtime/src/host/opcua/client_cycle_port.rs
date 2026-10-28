//! OPC UA cache boundaries at a compiled program's variables at the boundaries of its scan
//! (specification 44 §13.3).
use super::*;
use crate::runtime::GlobalPort;
use trust_runtime_core::native::variables::CompiledVariables;

impl OpcUaClientBridge {
    /// Applies the cached samples at the protocol-input boundary, after the drivers read the inputs.
    /// A rejected destination updates the point's quality and leaves the program executable.
    pub fn apply_inputs_compiled(
        &mut self,
        variables: &mut dyn CompiledVariables,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        self.apply_inputs_with(now_ms, variables)
    }

    /// Captures the program's values after its scan, keeping the unchanged-value suppression and
    /// the worker's generation-bound write acknowledgements.
    pub fn capture_outputs_compiled(
        &mut self,
        variables: &mut dyn CompiledVariables,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        self.capture_outputs_through(variables, now_ms)
    }

    fn capture_outputs_through(
        &mut self,
        port: &mut (impl GlobalPort + ?Sized),
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.point.access.can_write())
        {
            let value = match port.read(&binding.target) {
                Ok(value) => value,
                Err(error) => {
                    self.shared.set_point_status(
                        &binding.point,
                        OpcUaClientConnectionState::Faulted,
                        Some(now_ms),
                        None,
                        format!(
                            "Failed to read OPC UA output '{}': {} ({})",
                            binding.point.var,
                            error,
                            error.stable_code()
                        ),
                    );
                    propagate_execution_error(error)?;
                    continue;
                }
            };
            queue_output(
                &self.shared,
                &mut self.last_queued_values,
                binding,
                value,
                now_ms,
            )?;
        }
        Ok(())
    }
}

pub(super) fn propagate_execution_error(error: RuntimeError) -> Result<(), OpcUaClientBridgeError> {
    match error {
        RuntimeError::ExecutionTimeout
        | RuntimeError::ExecutionLimit { .. }
        | RuntimeError::WatchdogTimeout => Err(OpcUaClientBridgeError::execution(error)),
        _ => Ok(()),
    }
}

pub(super) fn queue_output(
    shared: &OpcUaSharedClientCache,
    previous: &mut std::collections::BTreeMap<SmolStr, Value>,
    binding: &OpcUaClientBinding,
    value: Value,
    now_ms: u64,
) -> Result<(), OpcUaClientBridgeError> {
    if previous
        .get(binding.point.var.as_str())
        .is_some_and(|last| last == &value)
    {
        return Ok(());
    }
    if !opcua_output_is_finite(&binding.point, &value) {
        shared.reject_output(
            &binding.point,
            now_ms,
            format!(
                "OPC UA output '{}' rejected non-finite {} value",
                binding.point.var,
                binding.point.data_type.as_config_value(),
            ),
        )?;
        previous.remove(binding.point.var.as_str());
        return Ok(());
    }
    shared.queue_write(&binding.point, value.clone())?;
    previous.insert(binding.point.var.clone(), value);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::runtime::NamedGlobals;

    fn bridge(
        target: crate::runtime::BoundGlobal,
        boolean: bool,
        access: OpcUaClientPointAccess,
        value: Value,
    ) -> OpcUaClientBridge {
        let point = OpcUaClientPointConfig {
            var: "point".into(),
            node_id: "ns=2;i=1".into(),
            data_type: if boolean {
                OpcUaDataType::Boolean
            } else {
                OpcUaDataType::Int16
            },
            access,
        };
        let bridge = OpcUaClientBridge::new(vec![OpcUaClientBinding {
            point: point.clone(),
            target,
        }])
        .unwrap();
        bridge
            .shared
            .apply_sample(OpcUaClientSample {
                var: point.var,
                node_id: point.node_id,
                data_type: point.data_type,
                access: point.access,
                value: Some(value),
                state: OpcUaClientConnectionState::Connected,
                last_seen_ms: Some(10),
                detail: "native cache sample".into(),
            })
            .unwrap();
        bridge
    }

    /// A boundary whose every access fails with its error.
    struct Refusing(RuntimeError);

    impl crate::runtime::GlobalPort for Refusing {
        fn read(&mut self, _target: &crate::runtime::BoundGlobal) -> Result<Value, RuntimeError> {
            Err(self.0.clone())
        }

        fn write(
            &mut self,
            _target: &crate::runtime::BoundGlobal,
            _value: Value,
        ) -> Result<(), RuntimeError> {
            Err(self.0.clone())
        }
    }

    #[test]
    fn input_cycle_limit_keeps_its_structured_cause_and_point_diagnostic() {
        let mut bridge = bridge(
            crate::runtime::BoundGlobal("probe_depth".into()),
            false,
            OpcUaClientPointAccess::Read,
            Value::Int(2),
        );
        let cause = RuntimeError::ExecutionLimit {
            kind: trust_runtime_core::error::ExecutionLimitKind::AllocationBytes,
            used: 8,
            requested: 4,
            limit: 8,
        };
        let error = bridge
            .apply_inputs_with(10, &mut Refusing(cause.clone()))
            .unwrap_err();
        assert_eq!(error.execution_cause(), Some(&cause));
        assert_eq!(
            std::error::Error::source(&error)
                .unwrap()
                .downcast_ref::<RuntimeError>(),
            Some(&cause)
        );
        let status = &bridge.snapshot().point_statuses[0];
        assert_eq!(status.state, OpcUaClientConnectionState::Faulted);
        assert!(status.detail.contains("runtime_execution_timeout"));
    }

    #[test]
    fn duplicate_capture_keeps_the_read_baseline_and_the_pending_generation() {
        let mut bridge = bridge(
            crate::runtime::BoundGlobal("probe_depth".into()),
            false,
            OpcUaClientPointAccess::ReadWrite,
            Value::Int(2),
        );
        let mut globals = NamedGlobals(std::collections::BTreeMap::from([(
            "probe_depth",
            Value::Int(1),
        )]));
        bridge.apply_inputs_compiled(&mut globals, 10).unwrap();
        bridge.capture_outputs_compiled(&mut globals, 10).unwrap();
        assert!(bridge.pending_write("point").is_none());
        globals.0.insert("probe_depth", Value::Int(4));
        bridge.capture_outputs_compiled(&mut globals, 10).unwrap();
        let generation = bridge.shared.pending_write_batch()["point"].generation;
        bridge.capture_outputs_compiled(&mut globals, 10).unwrap();
        assert_eq!(
            bridge.shared.pending_write_batch()["point"].generation,
            generation
        );
        assert_eq!(bridge.pending_write("point"), Some(Value::Int(4)));
    }

    /// Specification 44 §13.3 with specification 11's OPC UA client: a compiled program's point
    /// reaches its global by name. The connected sample 2 of the read-write point is written to the
    /// global `Speed` after the inputs, while `Other` keeps 0; after the scan, when `Speed` holds 7,
    /// 7 is queued for the server. Catches a compiled program whose inputs never reach its globals,
    /// whose outputs are never queued, or a point that reaches another global.
    #[test]
    fn a_compiled_programs_point_reaches_its_global_by_name() {
        let mut bridge = bridge(
            crate::runtime::BoundGlobal("Speed".into()),
            false,
            OpcUaClientPointAccess::ReadWrite,
            Value::Int(2),
        );
        let mut globals = NamedGlobals(std::collections::BTreeMap::from([
            ("Speed", Value::Int(0)),
            ("Other", Value::Int(0)),
        ]));

        bridge.apply_inputs_compiled(&mut globals, 10).unwrap();
        assert_eq!(globals.0["Speed"], Value::Int(2));
        assert_eq!(globals.0["Other"], Value::Int(0));

        globals.0.insert("Speed", Value::Int(7));
        bridge.capture_outputs_compiled(&mut globals, 20).unwrap();
        assert_eq!(bridge.pending_write("point"), Some(Value::Int(7)));
    }
}
