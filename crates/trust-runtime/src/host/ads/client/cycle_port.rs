//! ADS cache application at a compiled program's variables at the boundaries of its scan
//! (specification 44 §13.3).
use super::*;
use trust_runtime_core::native::variables::CompiledVariables;

impl AdsConnectionBridge {
    /// Applies the cached samples at the protocol-input boundary, after the drivers read the inputs.
    /// A rejected destination updates the point's quality and leaves the program executable.
    pub fn apply_inputs_compiled(
        &mut self,
        variables: &mut dyn CompiledVariables,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        self.apply_inputs_with(now_ms, variables)
    }

    /// Captures the program's values after its scan, keeping the unchanged-value suppression and
    /// the worker's generation-bound write acknowledgements.
    pub fn capture_outputs_compiled(
        &mut self,
        variables: &mut dyn CompiledVariables,
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        self.capture_outputs_through(variables, now_ms)
    }

    fn capture_outputs_through(
        &mut self,
        port: &mut (impl GlobalPort + ?Sized),
        now_ms: u64,
    ) -> Result<(), AdsBridgeError> {
        let snapshot = self.shared.snapshot();
        for binding in self
            .bindings
            .iter()
            .filter(|binding| point_writes(binding.point.access))
        {
            if let Some(quality) = snapshot.qualities.get(binding.point.point_name.as_str()) {
                self.write_quality_with(binding, quality, now_ms, port)?;
            }
            let value = match port.read(&binding.target) {
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
    use trust_ads_core::{PointAccess, UpdateMode};

    fn binding(access: PointAccess) -> AdsBinding {
        AdsBinding {
            point: AdsPointConfig {
                point_name: "point".into(),
                address: crate::ads::AdsPointAddress::Symbol("Remote.Value".into()),
                data_type: AdsDataTypeDescriptor::scalar("INT", IecDataType::Int),
                access,
                mode: UpdateMode::Poll,
                notification_mode: crate::ads::AdsNotificationMode::OnChange,
                allow_retain_read: false,
            },
            target: BoundGlobal("probe_depth".into()),
            quality_target: None,
            retain: false,
        }
    }

    /// A boundary whose every access fails with its error.
    struct Refusing(RuntimeError);

    impl GlobalPort for Refusing {
        fn read(&mut self, _target: &BoundGlobal) -> Result<Value, RuntimeError> {
            Err(self.0.clone())
        }

        fn write(&mut self, _target: &BoundGlobal, _value: Value) -> Result<(), RuntimeError> {
            Err(self.0.clone())
        }
    }

    /// A boundary over the program's one INT global, `probe_depth`.
    struct OneGlobal(Value);

    impl GlobalPort for OneGlobal {
        fn read(&mut self, _target: &BoundGlobal) -> Result<Value, RuntimeError> {
            Ok(self.0.clone())
        }

        fn write(&mut self, _target: &BoundGlobal, value: Value) -> Result<(), RuntimeError> {
            self.0 = value;
            Ok(())
        }
    }

    #[test]
    fn input_cycle_limit_keeps_its_structured_cause_and_point_diagnostic() {
        let mut bridge = AdsConnectionBridge::new(vec![binding(PointAccess::Read)]).unwrap();
        bridge.shared.set_value("point", Value::Int(2));
        bridge.shared.set_quality("point", PointQuality::good(10));
        let cause = RuntimeError::ExecutionLimit {
            kind: trust_runtime_core::error::ExecutionLimitKind::Work,
            used: 10,
            requested: 1,
            limit: 10,
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
        let quality = bridge.status("point").unwrap().quality;
        assert_eq!(quality.state, QualityState::Error);
        assert!(format!("{quality:?}").contains("runtime_execution_timeout"));
    }

    #[test]
    fn capture_keeps_the_read_baseline_and_the_write_generation_of_a_duplicate() {
        let mut bridge = AdsConnectionBridge::new(vec![binding(PointAccess::ReadWrite)]).unwrap();
        bridge.shared.set_value("point", Value::Int(2));
        bridge.shared.set_quality("point", PointQuality::good(10));
        let mut port = OneGlobal(Value::Int(1));
        bridge.apply_inputs_with(10, &mut port).unwrap();
        bridge.capture_outputs_through(&mut port, 10).unwrap();
        assert!(
            bridge.pending_write("point").is_none(),
            "read/write input establishes the unchanged output baseline"
        );
        port.0 = Value::Int(4);
        bridge.capture_outputs_through(&mut port, 10).unwrap();
        let first = bridge.shared.pending_write_batch().0["point"].generation;
        bridge.capture_outputs_through(&mut port, 10).unwrap();
        assert_eq!(
            bridge.shared.pending_write_batch().0["point"].generation,
            first
        );
        assert_eq!(bridge.pending_write("point"), Some(Value::Int(4)));
    }
}
