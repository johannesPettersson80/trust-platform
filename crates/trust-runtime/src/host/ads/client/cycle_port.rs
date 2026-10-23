//! ADS cache application through the authoritative checked cycle owner.
use super::*;
use trust_runtime_core::vm::CycleAccess;

impl AdsConnectionBridge {
    /// Apply cached samples at the protocol-input boundary. Rejected destinations
    /// update point quality and leave the PLC owner executable, as legacy bridges do.
    pub fn apply_inputs_with_cycle(
        &mut self,
        access: &mut CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        self.apply_inputs_with(now_ms, |reference, value| {
            access.write_reference(reference.as_view(), value)
        })
    }

    /// Capture PLC values after force application, preserving unchanged-value
    /// suppression and the worker's generation-bound write acknowledgements.
    pub fn capture_outputs_with_cycle(
        &mut self,
        access: &mut CycleAccess<'_, '_>,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| point_writes(binding.point.access))
        {
            if let Some(quality) = snapshot.qualities.get(binding.point.point_name.as_str()) {
                self.write_quality_with(binding, quality, now_ms, &mut |reference, value| {
                    access.write_reference(reference.as_view(), value)
                })?;
            }
            let value = match access.read_reference(&binding.reference) {
                Ok(value) => value,
                Err(error) => {
                    self.shared.set_quality(
                        binding.point.point_name.as_str(),
                        PointQuality::error(
                            now_ms,
                            format!(
                                "failed to read ADS output '{}': {} ({})",
                                binding.point.point_name,
                                error,
                                error.stable_code()
                            ),
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
            );
        }
        Ok(())
    }
}

// Point rejection leaves the PLC executable. Exhaustion of the cycle's work,
// allocation or physical deadline belongs to the enclosing resource fault owner.
pub(super) fn propagate_execution_error(error: RuntimeError) -> Result<(), AdsBridgeError> {
    match error {
        RuntimeError::ExecutionTimeout
        | RuntimeError::ExecutionLimit { .. }
        | RuntimeError::WatchdogTimeout => Err(AdsBridgeError::execution(error)),
        _ => Ok(()),
    }
}

pub(super) fn queue_output(
    shared: &AdsSharedCache,
    previous: &mut BTreeMap<String, Value>,
    binding: &AdsBinding,
    value: Value,
    now_ms: u64,
) {
    if contains_non_finite_float(&value) {
        shared.reject_write(
            binding.point.point_name.as_str(),
            now_ms,
            format!(
                "ADS output '{}' contains a non-finite REAL/LREAL value",
                binding.point.point_name,
            ),
        );
        previous.remove(binding.point.point_name.as_str());
        return;
    }
    if previous
        .get(binding.point.point_name.as_str())
        .is_some_and(|last| last == &value)
    {
        return;
    }
    shared.queue_write(binding.point.point_name.clone(), value.clone());
    previous.insert(binding.point.point_name.clone(), value);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::Duration;
    use trust_ads_core::{PointAccess, UpdateMode};
    use trust_runtime_core::vm::{CycleHooks, CyclePhase, PreparationLimits, PreparedModule};

    const GPIO: &[u8] = include_bytes!("../../../../tests/fixtures/portability/f401/gpio.stbc");

    fn binding(reference: ValueRef, access: PointAccess, boolean: bool) -> AdsBinding {
        AdsBinding {
            point: AdsPointConfig {
                point_name: "point".into(),
                address: crate::ads::AdsPointAddress::Symbol("Remote.Value".into()),
                data_type: AdsDataTypeDescriptor::scalar(
                    if boolean { "BOOL" } else { "INT" },
                    if boolean {
                        IecDataType::Bool
                    } else {
                        IecDataType::Int
                    },
                ),
                access,
                mode: UpdateMode::Poll,
                notification_mode: crate::ads::AdsNotificationMode::OnChange,
                allow_retain_read: false,
            },
            reference,
            quality_reference: None,
            type_id: if boolean { TypeId::BOOL } else { TypeId::INT },
            retain: false,
        }
    }

    #[test]
    fn input_cycle_limit_keeps_its_structured_cause_and_point_diagnostic() {
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        let state = prepared.instantiate(0).unwrap();
        let mut bridge = AdsConnectionBridge::new(vec![binding(
            state.storage().ref_for_global("probe_depth").unwrap(),
            PointAccess::Read,
            false,
        )])
        .unwrap();
        bridge.shared.set_value("point", Value::Int(2));
        bridge.shared.set_quality("point", PointQuality::good(10));
        let cause = RuntimeError::ExecutionLimit {
            kind: trust_runtime_core::error::ExecutionLimitKind::Work,
            used: 10,
            requested: 1,
            limit: 10,
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
        let quality = bridge.status("point").unwrap().quality;
        assert_eq!(quality.state, QualityState::Error);
        assert!(format!("{quality:?}").contains("runtime_execution_timeout"));
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
            bridge: AdsConnectionBridge,
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
            let bridge = AdsConnectionBridge::new(vec![binding(
                state.storage().ref_for_global("probe_depth").unwrap(),
                PointAccess::ReadWrite,
                false,
            )])
            .unwrap();
            bridge.shared.set_value("point", Value::Int(2));
            bridge.shared.set_quality("point", PointQuality::good(10));
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
    fn checked_ads_samples_follow_mapped_inputs_and_rejections_are_point_errors() {
        struct Hooks {
            bridge: AdsConnectionBridge,
            rejects: bool,
        }
        impl CycleHooks for Hooks {
            fn on_phase(
                &mut self,
                phase: CyclePhase,
                access: &mut CycleAccess<'_, '_>,
            ) -> Result<(), crate::error::RuntimeError> {
                if phase == CyclePhase::BeforeInputSample {
                    access.inputs_mut()?[0] = 1;
                }
                if phase == CyclePhase::AfterInputSample {
                    if !self.rejects {
                        let Some(Value::Instance(plant)) = access.storage().get_global("Plant")
                        else {
                            panic!("root")
                        };
                        assert_eq!(
                            access.storage().get_instance_var(*plant, "button"),
                            Some(&Value::Bool(true))
                        );
                    }
                    self.bridge.apply_inputs_with_cycle(access, 10).unwrap();
                }
                Ok(())
            }
        }
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        for rejects in [false, true] {
            let mut state = prepared.instantiate(0).unwrap();
            let reference = if rejects {
                state.storage().ref_for_global("probe_depth").unwrap()
            } else {
                let Some(Value::Instance(plant)) = state.storage().get_global("Plant") else {
                    panic!("root")
                };
                state.storage().ref_for_instance(*plant, "button").unwrap()
            };
            let bridge =
                AdsConnectionBridge::new(vec![binding(reference, PointAccess::Read, true)])
                    .unwrap();
            bridge.shared.set_value("point", Value::Bool(false));
            bridge.shared.set_quality("point", PointQuality::good(10));
            let mut hooks = Hooks { bridge, rejects };
            state
                .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
                .unwrap();
            assert!(state.fault().is_none());
            if rejects {
                assert_eq!(
                    state.storage().get_global("probe_depth"),
                    Some(&Value::Int(1))
                );
                assert_eq!(
                    hooks.bridge.status("point").unwrap().quality.state,
                    QualityState::Error
                );
            } else {
                assert_eq!(
                    state.outputs()[0] & 1,
                    0,
                    "ADS inputs override mapped input values before PLC execution"
                );
            }
        }
    }

    #[test]
    fn checked_ads_capture_observes_forced_storage_and_preserves_write_generation_on_duplicates() {
        struct Hooks(AdsConnectionBridge);
        impl CycleHooks for Hooks {
            fn on_phase(
                &mut self,
                phase: CyclePhase,
                access: &mut CycleAccess<'_, '_>,
            ) -> Result<(), crate::error::RuntimeError> {
                if phase == CyclePhase::AfterInputSample {
                    self.0.apply_inputs_with_cycle(access, 10).unwrap();
                }
                if phase == CyclePhase::AfterOutputSample {
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    assert!(
                        self.0.pending_write("point").is_none(),
                        "read/write input establishes the unchanged output baseline"
                    );
                    access.write_named_path("probe_depth", &[], Value::Int(4))?;
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    let first = self.0.shared.pending_write_batch().0["point"].generation;
                    self.0.capture_outputs_with_cycle(access, 10).unwrap();
                    assert_eq!(
                        self.0.shared.pending_write_batch().0["point"].generation,
                        first
                    );
                }
                Ok(())
            }
        }
        let prepared = PreparedModule::from_bytes(GPIO, PreparationLimits::default()).unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        let bridge = AdsConnectionBridge::new(vec![binding(
            state.storage().ref_for_global("probe_depth").unwrap(),
            PointAccess::ReadWrite,
            false,
        )])
        .unwrap();
        bridge.shared.set_value("point", Value::Int(2));
        bridge.shared.set_quality("point", PointQuality::good(10));
        let mut hooks = Hooks(bridge);
        state
            .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
            .unwrap();
        assert_eq!(hooks.0.pending_write("point"), Some(Value::Int(4)));
        assert!(state.fault().is_none());
    }
}
