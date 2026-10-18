//! Conservative construction/snapshot charges; physical peaks are measured at board bring-up.
use super::*;
use crate::error::ExecutionLimitKind;
use crate::memory::layout::CELL_BYTES;

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

    /// The charge for one copy of `value` (TIME-08): its work is charged here, and its
    /// allocation is returned for the caller to charge. Neither depends on how the engine shares
    /// the value, so a copy charges the same whether its storage is shared or not.
    pub(in crate::vm::engine) fn value_clone_charge(
        &self,
        value: &Value,
        depth: usize,
    ) -> Result<usize, RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        self.charge_copy_work(value, depth)?;
        copy_allocation(value, depth)
    }

    /// The work of copying `value` (TIME-08 COPY). One rule covers arrays and structures: one unit
    /// for each array and each structure, plus one for each of its elements or fields, at every
    /// nesting level. Each leaf then pays its own row's charge: a WSTRING or an enumeration one
    /// unit, a reference one unit plus its path. A scalar, a STRING or a null reference pays nothing.
    fn charge_copy_work(&self, value: &Value, depth: usize) -> Result<(), RuntimeError> {
        if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
            return Err(RuntimeError::TypeMismatch);
        }
        match value {
            Value::WString(_) | Value::Enum(_) => self.charge_work_units(1),
            Value::Reference(Some(reference)) => {
                self.charge_work_units(1)?;
                self.charge_reference_path_work(&reference.path)
            }
            Value::Array(array) => {
                self.charge_work_units(1)?;
                self.charge_work_units(array.elements().len())?;
                for element in array.elements() {
                    self.charge_copy_work(element, depth + 1)?;
                }
                Ok(())
            }
            Value::Struct(structure) => {
                self.charge_work_units(1)?;
                self.charge_work_units(structure.fields().len())?;
                for field in structure.fields().values() {
                    self.charge_copy_work(field, depth + 1)?;
                }
                Ok(())
            }
            _ => Ok(()),
        }
    }

    /// The work of a reference path's copy: one unit per segment and one per subscript.
    fn charge_reference_path_work(
        &self,
        path: &[crate::value::RefSegment],
    ) -> Result<(), RuntimeError> {
        self.charge_work_units(path.len())?;
        for segment in path {
            if let crate::value::RefSegment::Index(indices) = segment {
                self.charge_work_units(indices.len())?;
            }
        }
        Ok(())
    }

    /// The work and allocation of a reference path's copy (TIME-08 COPY).
    pub(super) fn reference_path_clone_charge(
        &self,
        path: &[crate::value::RefSegment],
    ) -> Result<usize, RuntimeError> {
        self.charge_reference_path_work(path)?;
        reference_path_allocation(path)
    }
}

/// The allocation of one copy of `value` (TIME-08 COPY): a scalar allocates nothing, a STRING its
/// UTF-8 bytes, a WSTRING two bytes per UTF-16 code unit, a reference one cell per path element,
/// and a structure or an array one cell for each copied leaf, counted through nesting.
pub(in crate::vm::engine) fn copy_allocation(
    value: &Value,
    depth: usize,
) -> Result<usize, RuntimeError> {
    if depth > crate::bytecode::BYTECODE_MAX_TYPE_NESTING {
        return Err(RuntimeError::TypeMismatch);
    }
    match value {
        Value::String(text) => text_copy_allocation(text.as_str(), false),
        Value::WString(text) => text_copy_allocation(text.as_str(), true),
        Value::Reference(Some(reference)) => reference_path_allocation(&reference.path),
        Value::Array(array) => leaves_allocation(array.elements().iter(), depth),
        Value::Struct(structure) => leaves_allocation(structure.fields().values(), depth),
        _ => Ok(0),
    }
}

/// The copy allocation of a text (TIME-08): its UTF-8 bytes for a STRING, and two bytes per UTF-16
/// code unit for a WSTRING, the stored form. A text cut at a store charges its kept prefix with
/// the same rule.
pub(in crate::vm::engine) fn text_copy_allocation(
    text: &str,
    wide: bool,
) -> Result<usize, RuntimeError> {
    if wide {
        text.encode_utf16()
            .count()
            .checked_mul(2)
            .ok_or(RuntimeError::Overflow)
    } else {
        Ok(text.len())
    }
}

/// The allocation of the children of a copied structure or array: one cell for each leaf child,
/// and for a compound child the allocation of its own leaves (TIME-08 COPY).
fn leaves_allocation<'a>(
    children: impl Iterator<Item = &'a Value>,
    depth: usize,
) -> Result<usize, RuntimeError> {
    let mut bytes: usize = 0;
    for child in children {
        let child_bytes = match child {
            Value::Array(_) | Value::Struct(_) => copy_allocation(child, depth + 1)?,
            _ => CELL_BYTES
                .checked_add(copy_allocation(child, depth + 1)?)
                .ok_or(RuntimeError::Overflow)?,
        };
        bytes = bytes
            .checked_add(child_bytes)
            .ok_or(RuntimeError::Overflow)?;
    }
    Ok(bytes)
}

/// One cell for each segment and for each subscript of a reference path (TIME-08 COPY).
fn reference_path_allocation(path: &[crate::value::RefSegment]) -> Result<usize, RuntimeError> {
    let mut elements = path.len();
    for segment in path {
        if let crate::value::RefSegment::Index(indices) = segment {
            elements = elements
                .checked_add(indices.len())
                .ok_or(RuntimeError::Overflow)?;
        }
    }
    elements
        .checked_mul(CELL_BYTES)
        .ok_or(RuntimeError::Overflow)
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
        let reference = Value::Reference(Some(ValueRef {
            location: MemoryLocation::Global,
            offset: 0,
            generation: data.storage.generation(),
            path: vec![
                RefSegment::Field("field".into()),
                RefSegment::Index(vec![4, 5]),
            ],
        }));
        // TIME-08 COPY: the path has 2 segments and 2 subscripts, one 16-byte cell each: 4 x 16 = 64.
        prepared.limits.max_construction_bytes = 64;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(10);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        state.before_value_clone(&Value::Int(7)).unwrap();

        // TIME-08: a scalar copy is instruction-only and allocates nothing.
        assert_eq!(state.data.resources.work_budget.remaining(), 10);
        assert_eq!(state.data.resources.construction_bytes.get(), 0);

        state.before_value_clone(&reference).unwrap();
        // TIME-08 COPY: reference 1 + 2 path segments + 2 indices = 5.
        assert_eq!(state.data.resources.work_budget.remaining(), 5);
        assert_eq!(state.data.resources.construction_bytes.get(), 64);
    }

    #[test]
    fn copy_before_value_clone_preserves_work_deadline_and_allocation_order() {
        use crate::vm::context::ReferenceContext;
        use core::cell::Cell;

        let mut prepared = PreparedModule::from_bytes(
            include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
            crate::vm::PreparationLimits::default(),
        )
        .unwrap();
        let mut data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        // TIME-08 COPY: the text "ab" copies 2 UTF-8 bytes and no work, so a limit of 1 refuses it.
        prepared.limits.max_construction_bytes = 1;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(1);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

        assert_eq!(
            state.before_value_clone(&Value::String("ab".into())),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used: 0,
                requested: 2,
                limit: 1,
            })
        );
        // TIME-08: the refused text copy consumed no work before its allocation check.
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
            Ok(0)
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
    fn nested_array_copy_charges_one_cell_per_leaf_at_any_depth() {
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
        // TIME-08 COPY: the one leaf, the INT 9, costs one 16-byte cell. The arrays that nest it
        // allocate nothing of their own, so a copy of 16 bytes is refused by a limit of 15.
        prepared.limits.max_construction_bytes = 15;
        data.resources.construction_bytes.set(0);
        data.resources.work_budget.reset(prepared.limits.max_work);
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        assert_eq!(
            state.before_value_clone(&input),
            Err(RuntimeError::ExecutionLimit {
                kind: ExecutionLimitKind::AllocationBytes,
                used: 0,
                requested: 16,
                limit: 15,
            })
        );
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
