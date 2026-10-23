//! OPC UA cache boundaries using one checked PLC execution owner.
use super::*;
use trust_runtime_core::vm::CycleAccess;

impl OpcUaClientBridge {
    /// Apply cached samples after process-image sampling. A rejected destination
    /// becomes a point status error rather than a PLC execution fault.
    pub fn apply_inputs_with_cycle(
        &mut self,
        access: &mut CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        self.apply_inputs_with(now_ms, |reference, value| {
            access.write_reference(reference.as_view(), value)
        })
    }

    /// Capture current PLC values after forces. Queue acknowledgement/generation
    /// handling stays with the existing worker and shared cache.
    pub fn capture_outputs_with_cycle(
        &mut self,
        access: &mut CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), OpcUaClientBridgeError> {
        for binding in self
            .bindings
            .iter()
            .filter(|binding| binding.point.access.can_write())
        {
            let value = match access.read_reference(&binding.reference) {
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
    use crate::value::{Duration, ValueRef};
    use trust_runtime_core::vm::{CycleHooks, CyclePhase, PreparationLimits, PreparedModule};

    const GPIO: &[u8] = include_bytes!("../../../tests/fixtures/portability/f401/gpio.stbc");

    fn bridge(
        reference: ValueRef,
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
            reference,
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

    #[test]
    fn input_cycle_limit_keeps_its_structured_cause_and_point_diagnostic() {
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        let state = prepared.instantiate(0).unwrap();
        let mut bridge = bridge(
            state.storage().ref_for_global("probe_depth").unwrap(),
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
            .apply_inputs_with(10, |_, _| Err(cause.clone()))
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
    fn physical_expiry_in_each_protocol_boundary_reaches_the_cycle_fault_owner() {
        struct Deadline(std::cell::Cell<bool>);
        impl trust_runtime_core::vm::ExecutionServices for Deadline {
            fn deadline_exceeded(&self) -> bool {
                self.0.get()
            }
            fn has_wall_clock(&self) -> bool {
                false
            }
            fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
                Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
            }
        }
        struct Hooks<'a> {
            bridge: OpcUaClientBridge,
            deadline: &'a Deadline,
            phase: CyclePhase,
        }
        impl CycleHooks for Hooks<'_> {
            fn on_phase(
                &mut self,
                phase: CyclePhase,
                access: &mut CycleAccess<'_, '_>,
            ) -> Result<(), RuntimeError> {
                if phase != self.phase {
                    return Ok(());
                }
                self.deadline.0.set(true);
                let result = if phase == CyclePhase::AfterInputSample {
                    self.bridge.apply_inputs_with_cycle(access, 10)
                } else {
                    self.bridge.capture_outputs_with_cycle(access, 10)
                };
                result.map_err(|error| {
                    error
                        .execution_cause()
                        .cloned()
                        .expect("typed execution cause")
                })
            }
        }
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        for phase in [CyclePhase::AfterInputSample, CyclePhase::AfterOutputSample] {
            let deadline = Deadline(std::cell::Cell::new(false));
            let mut state = prepared.instantiate_with_services(0, &deadline).unwrap();
            let bridge = bridge(
                state.storage().ref_for_global("probe_depth").unwrap(),
                false,
                OpcUaClientPointAccess::ReadWrite,
                Value::Int(2),
            );
            let mut hooks = Hooks {
                bridge,
                deadline: &deadline,
                phase,
            };
            assert_eq!(
                state.execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks),
                Err(RuntimeError::ExecutionTimeout)
            );
            assert_eq!(state.fault(), Some(&RuntimeError::ExecutionTimeout));
            assert!(hooks.bridge.pending_write("point").is_none());
        }
    }

    #[test]
    fn checked_opcua_inputs_override_image_samples_and_reject_changed_destination_types_without_faulting_plc(
    ) {
        struct Hooks(OpcUaClientBridge);
        impl CycleHooks for Hooks {
            fn on_phase(
                &mut self,
                phase: CyclePhase,
                access: &mut CycleAccess<'_, '_>,
            ) -> Result<(), RuntimeError> {
                if phase == CyclePhase::BeforeInputSample {
                    access.inputs_mut()?[0] = 1;
                }
                if phase == CyclePhase::AfterInputSample {
                    self.0.apply_inputs_with_cycle(access, 10).unwrap();
                }
                Ok(())
            }
        }
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        for rejected in [false, true] {
            let mut state = prepared.instantiate(0).unwrap();
            let reference = if rejected {
                state.storage().ref_for_global("probe_depth").unwrap()
            } else {
                let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
                    panic!("root")
                };
                state.storage().ref_for_instance(*plant, "button").unwrap()
            };
            let mut hooks = Hooks(bridge(
                reference,
                true,
                OpcUaClientPointAccess::Read,
                Value::Bool(false),
            ));
            state
                .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
                .unwrap();
            assert!(state.fault().is_none());
            if rejected {
                assert_eq!(
                    state.storage().get_global("probe_depth"),
                    Some(&Value::Int(1))
                );
                assert_eq!(
                    hooks.0.snapshot().point_statuses[0].state,
                    OpcUaClientConnectionState::Faulted
                );
            } else {
                assert_eq!(
                    state.outputs()[0] & 1,
                    0,
                    "protocol sampling follows physical mapped input sampling"
                );
            }
        }
    }

    #[test]
    fn checked_opcua_outputs_use_post_force_values_and_duplicate_capture_keeps_pending_generation()
    {
        struct Hooks(OpcUaClientBridge);
        impl CycleHooks for Hooks {
            fn on_phase(
                &mut self,
                phase: CyclePhase,
                access: &mut CycleAccess<'_, '_>,
            ) -> Result<(), RuntimeError> {
                if phase == CyclePhase::AfterInputSample {
                    self.0.apply_inputs_with_cycle(access, 10).unwrap();
                }
                if phase == CyclePhase::AfterOutputSample {
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    assert!(self.0.pending_write("point").is_none());
                    access.write_named_path("probe_depth", &[], Value::Int(4))?;
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    let generation = self.0.shared.pending_write_batch()["point"].generation;
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    assert_eq!(
                        self.0.shared.pending_write_batch()["point"].generation,
                        generation
                    );
                }
                Ok(())
            }
        }
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        let mut hooks = Hooks(bridge(
            state.storage().ref_for_global("probe_depth").unwrap(),
            false,
            OpcUaClientPointAccess::ReadWrite,
            Value::Int(2),
        ));
        state
            .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
            .unwrap();
        assert_eq!(hooks.0.pending_write("point"), Some(Value::Int(4)));
        assert!(state.fault().is_none());
    }
}
