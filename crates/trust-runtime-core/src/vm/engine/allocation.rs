//! Conservative construction/snapshot charges; physical peaks are measured at board bring-up.
use super::*;
use crate::error::ExecutionLimitKind;

impl EngineState<'_> {
    pub(in crate::vm::engine) fn charge_constructed_value(&mut self) -> Result<(), RuntimeError> {
        self.charge_constructed_values(1)
    }
    pub(in crate::vm::engine) fn charge_constructed_values(
        &mut self,
        units: usize,
    ) -> Result<(), RuntimeError> {
        let used = self.data.resources.constructed_values;
        let limit = self.prepared.limits.max_construction_values;
        let Some(count) = used.checked_add(units).filter(|total| *total <= limit) else {
            return Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::ConstructedValues,
                used,
                requested: units,
                limit,
            });
        };
        self.data.resources.constructed_values = count;
        Ok(())
    }

    pub(in crate::vm::engine) fn charge_allocation_bytes(
        &self,
        bytes: usize,
    ) -> Result<(), RuntimeError> {
        let used = self.data.resources.construction_bytes.get();
        let limit = self.prepared.limits.max_construction_bytes;
        let Some(total) = used.checked_add(bytes).filter(|total| *total <= limit) else {
            return Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used,
                requested: bytes,
                limit,
            });
        };
        self.data.resources.construction_bytes.set(total);
        Ok(())
    }

    pub(in crate::vm::engine) fn value_clone_charge(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<usize, RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        // Most register values have no independently cloned backing allocation.
        // Keep their accounting here without entering the recursive estimator.
        match value {
            Value::WString(_) | Value::Array(_) | Value::Enum(_) | Value::Reference(Some(_)) => {
                // TIME-08 COPY: one unit for each owned copy.
                self.charge_work_units(1)?;
                self.owned_value_clone_charge(value, depth)
            }
            _ => Ok(core::mem::size_of::<Value>()),
        }
    }

    fn owned_value_clone_charge(&self, value: &Value, depth: usize) -> Result<usize, RuntimeError> {
        let mut bytes = core::mem::size_of::<Value>();
        match value {
            Value::WString(value) => {
                bytes = bytes
                    .checked_add(value.len())
                    .ok_or_else(|| RuntimeError::Overflow)?
            }
            Value::Array(array) => {
                bytes = bytes
                    .checked_add(core::mem::size_of::<crate::value::ArrayValue>())
                    .ok_or_else(|| RuntimeError::Overflow)?
                    .checked_add(
                        array
                            .dimensions()
                            .len()
                            .checked_mul(core::mem::size_of::<(i64, i64)>())
                            .ok_or_else(|| RuntimeError::Overflow)?,
                    )
                    .ok_or_else(|| RuntimeError::Overflow)?;
                // TIME-08 COPY: one unit per element; nested values charge themselves.
                self.charge_work_units(array.elements().len())?;
                for value in array.elements() {
                    bytes = bytes
                        .checked_add(self.value_clone_charge(value, depth + 1)?)
                        .ok_or_else(|| RuntimeError::Overflow)?;
                }
            }
            Value::Enum(_) => {
                bytes = bytes
                    .checked_add(core::mem::size_of::<crate::value::EnumValue>())
                    .ok_or_else(|| RuntimeError::Overflow)?
            }
            Value::Reference(Some(reference)) => {
                bytes = bytes
                    .checked_add(self.reference_path_clone_charge(&reference.path)?)
                    .ok_or_else(|| RuntimeError::Overflow)?;
            }
            _ => {}
        }
        Ok(bytes)
    }

    pub(super) fn reference_path_clone_charge(
        &self,
        path: &[crate::value::RefSegment],
    ) -> Result<usize, RuntimeError> {
        self.charge_work_units(path.len())?;
        let mut bytes = path
            .len()
            .checked_mul(core::mem::size_of::<crate::value::RefSegment>())
            .ok_or_else(|| RuntimeError::Overflow)?;
        for segment in path {
            if let crate::value::RefSegment::Index(indices) = segment {
                self.charge_work_units(indices.len())?;
                bytes = bytes
                    .checked_add(
                        indices
                            .len()
                            .checked_mul(core::mem::size_of::<i64>())
                            .ok_or_else(|| RuntimeError::Overflow)?,
                    )
                    .ok_or_else(|| RuntimeError::Overflow)?;
            }
        }
        Ok(bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn before_value_clone_accounts_simple_and_owned_paths() {
        use crate::memory::MemoryLocation;
        use crate::value::{RefSegment, ValueRef};
        use crate::vm::context::ReferenceContext;
        use alloc::vec;

        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let bytes = core::mem::size_of::<Value>();
        let reference = Value::Reference(Some(ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            path: vec![
                RefSegment::Field("field".into()),
                RefSegment::Index(vec![4, 5]),
            ],
        }));
        let reference_bytes = core::mem::size_of::<Value>()
            + 2 * core::mem::size_of::<RefSegment>()
            + 2 * core::mem::size_of::<i64>();
        prepared.limits.max_construction_bytes = bytes + reference_bytes;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(10);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        state.before_value_clone(&Value::Int(7)).unwrap();

        // TIME-08: a scalar copy is instruction-only and charges no work.
        assert_eq!(state.data.resources.work_budget.remaining(), 10);
        assert_eq!(state.data.resources.construction_bytes.get(), bytes);

        state.before_value_clone(&reference).unwrap();
        // TIME-08 COPY: reference 1 + 2 path segments + 2 indices = 5.
        assert_eq!(state.data.resources.work_budget.remaining(), 5);
        assert_eq!(
            state.data.resources.construction_bytes.get(),
            bytes + reference_bytes
        );
    }

    #[test]
    fn scalar_before_value_clone_preserves_work_deadline_and_allocation_order() {
        use crate::vm::context::ReferenceContext;
        use core::cell::Cell;

        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let bytes = core::mem::size_of::<Value>();
        prepared.limits.max_construction_bytes = bytes - 1;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(1);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        assert_eq!(
            state.before_value_clone(&Value::Int(7)),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used: 0,
                requested: bytes,
                limit: bytes - 1,
            })
        );
        // TIME-08: the refused scalar copy consumed no work before its allocation check.
        assert_eq!(state.data.resources.work_budget.remaining(), 1);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);

        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        prepared.limits.max_construction_bytes = 0;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(0);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        // TIME-08 COPY: a WSTRING copy is one unit, so it reaches the work refusal first.
        assert!(matches!(
            state.before_value_clone(&Value::WString(alloc::string::String::new())),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                used: 0,
                requested: 1,
                limit: 0,
            })
        ));
        assert_eq!(state.data.resources.work_budget.remaining(), 0);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);

        struct DeadlineProbe {
            calls: Cell<usize>,
            expired: Cell<bool>,
        }
        impl ExecutionServices for DeadlineProbe {
            fn deadline_exceeded(&self) -> bool {
                self.calls.set(self.calls.get() + 1);
                self.expired.get()
            }
            fn has_wall_clock(&self) -> bool {
                false
            }
            fn current_dt(&self) -> Result<crate::value::DateTimeValue, RuntimeError> {
                Err(RuntimeError::TypeMismatch)
            }
        }

        let prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let probe = DeadlineProbe {
            calls: Cell::new(0),
            expired: Cell::new(false),
        };
        let mut data = EngineState::construct_data(&prepared, 0, &probe).unwrap();
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(64);
        let state = EngineState::borrow(&mut data, &prepared, &probe);
        for _ in 0..31 {
            state.charge_work_units(1).unwrap();
        }
        probe.calls.set(0);
        probe.expired.set(true);

        // The WSTRING copy's one unit reaches the stride boundary and samples the deadline.
        assert_eq!(
            state.before_value_clone(&Value::WString(alloc::string::String::new())),
            Err(crate::vm::VmTrap::DeadlineExceeded.into_runtime_error())
        );
        assert_eq!(probe.calls.get(), 1);
        assert_eq!(state.data.resources.work_budget.remaining(), 32);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
    }

    #[test]
    fn scalar_clone_accounting_keeps_work_and_structural_depth_boundaries() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        prepared.limits.max_call_depth = 1;
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.data.resources.construction_bytes.set(0);
        state.data.resources.work_budget.reset(2);
        assert_eq!(
            state.value_clone_charge(&Value::Int(7), crate::bytecode::BYTECODE_MAX_TYPE_NESTING),
            Ok(core::mem::size_of::<Value>())
        );
        // TIME-08: scalar copies charge nothing, so the allowance is unchanged.
        assert_eq!(state.data.resources.work_budget.remaining(), 2);
        assert_eq!(
            state.value_clone_charge(
                &Value::Int(7),
                crate::bytecode::BYTECODE_MAX_TYPE_NESTING + 1
            ),
            Err(RuntimeError::TypeMismatch)
        );
        assert_eq!(state.data.resources.work_budget.remaining(), 2);
        // TIME-08 COPY: a WSTRING copy is one unit, refused by an empty allowance.
        state.data.resources.work_budget.reset(0);
        assert!(matches!(
            state.value_clone_charge(&Value::WString(alloc::string::String::new()), 0),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::Work,
                ..
            })
        ));
        assert_eq!(state.data.resources.work_budget.remaining(), 0);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
    }

    #[test]
    fn nested_array_clone_cannot_omit_each_separate_box_allocation() {
        use crate::value::ArrayValue;
        use crate::vm::context::ReferenceContext;
        use alloc::boxed::Box;
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let mut input = Value::Int(9);
        const DEPTH: usize = 3;
        for _ in 0..DEPTH {
            input = Value::Array(Box::new(ArrayValue::from_canonical_parts(
                vec![input],
                vec![(0, 0)],
            )));
        }
        // Each level necessarily clones a Box<ArrayValue>, a dimension Vec and
        // one element slot. This is a lower bound on real requested allocations,
        // not the implementation's conservative charge formula.
        let required = DEPTH
            * (core::mem::size_of::<ArrayValue>()
                + core::mem::size_of::<(i64, i64)>()
                + core::mem::size_of::<Value>());
        prepared.limits.max_construction_bytes = required - 1;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(prepared.limits.max_work);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        let error = state.before_value_clone(&input).unwrap_err();
        assert!(matches!(error, RuntimeError::ExecutionLimit {
            kind: ExecutionLimitKind::AllocationBytes, used: 0, requested, limit,
        } if requested >= required && limit == required - 1));
        assert_eq!(state.data.resources.construction_bytes.get(), 0);
    }

    #[test]
    fn value_and_backing_limits_report_their_own_first_refused_charge() {
        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        prepared.limits.max_construction_values = 8;
        prepared.limits.max_construction_bytes = 40;
        data.resources.constructed_values = 6;
        data.resources.construction_bytes.set(30);
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        assert_eq!(
            state.charge_constructed_values(3),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::ConstructedValues,
                used: 6,
                requested: 3,
                limit: 8,
            })
        );
        assert_eq!(state.data.resources.constructed_values, 6);
        assert_eq!(
            state.charge_allocation_bytes(11),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used: 30,
                requested: 11,
                limit: 40,
            })
        );
        assert_eq!(state.data.resources.construction_bytes.get(), 30);
        state.charge_constructed_values(2).unwrap();
        state.charge_allocation_bytes(10).unwrap();
    }
}

#[cfg(test)]
#[path = "allocation/clone_admission_tests.rs"]
mod clone_admission_tests;
#[cfg(test)]
#[path = "allocation/work_allowance_clone_tests.rs"]
mod work_allowance_clone_tests;
