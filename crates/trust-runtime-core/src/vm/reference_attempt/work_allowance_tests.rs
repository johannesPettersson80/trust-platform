//! TIME-08: type-structure comparisons are bookkeeping, so a cyclic type path draws no work
//! on the reference attempt that compares it.
use super::*;
use crate::bytecode::{TypeEntry, TypeTable};
use crate::error::RuntimeError;
use crate::memory::{FrameId, MemoryLocation, VariableStorage};
use crate::value::{write_value_path, RefSegment, ValueRef, ValueRefView};
use crate::vm::{
    budget::ExecutionBudget, context::ReferenceContext, PreparationLimits, PreparedModule,
};
use alloc::{vec, vec::Vec};

/// Two reference types that name each other: type 0 refers to 0, type 1 refers to 1.
/// Comparing them never reaches a match, so the bounded walk fails closed.
fn cyclic_reference_module() -> VmModule {
    let prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        PreparationLimits::default(),
    )
    .unwrap();
    let mut module = prepared.vm;
    module.types = TypeTable {
        offsets: vec![],
        entries: vec![
            TypeEntry {
                kind: TypeKind::Reference,
                name_idx: None,
                data: TypeData::Reference { target_type_id: 0 },
            },
            TypeEntry {
                kind: TypeKind::Reference,
                name_idx: None,
                data: TypeData::Reference { target_type_id: 1 },
            },
        ],
    };
    // The local reference at offset 0 has the first cyclic type.
    module.ref_types = vec![Some(0)];
    module
}

struct AttemptContext {
    storage: VariableStorage,
    budget: ExecutionBudget,
}

impl AttemptContext {
    fn new(limit: usize) -> Self {
        Self {
            storage: VariableStorage::new(),
            budget: ExecutionBudget::new(limit),
        }
    }
}

impl ReferenceContext for AttemptContext {
    fn before_reference_path_growth(&self, _bytes: usize, work: usize) -> Result<(), RuntimeError> {
        self.budget.charge(work).map(|_| ())
    }
    fn check_reference_write(
        &self,
        _reference: ValueRefView<'_>,
        _value: &Value,
    ) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn check_reference_read(&self, _reference: ValueRefView<'_>) -> Result<(), RuntimeError> {
        Ok(())
    }
    fn initializer_reference<'a>(
        &self,
        _initializer_id: u32,
        _path: &'a [RefSegment],
    ) -> Result<ValueRefView<'a>, VmTrap> {
        Err(VmTrap::NullReference)
    }
    fn storage(&self) -> &VariableStorage {
        &self.storage
    }
    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }
    fn write_typed_storage(
        &mut self,
        location: MemoryLocation,
        offset: usize,
        path: &[RefSegment],
        value: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(self
            .storage
            .write_by_ref_parts(location, offset, path, value))
    }
    fn write_typed_local(&self, root: &mut Value, path: &[RefSegment], value: Value) -> bool {
        write_value_path(root, path, value)
    }
}

#[test]
fn cyclic_type_comparison_in_a_reference_attempt_draws_no_work() {
    let module = cyclic_reference_module();
    let local = ValueRef {
        location: MemoryLocation::Local(FrameId(7)),
        offset: 0,
        path: Vec::new(),
    };
    let frame = VmFrame {
        activation: Some(FrameId(7)),
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 1,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };
    // Target type 1 is a reference to type 1, so the walk from type 0 never matches. The
    // comparison is bookkeeping: an empty allowance admits the attempt and draws nothing.
    let context = AttemptContext::new(0);
    assert_eq!(
        apply_reference_attempt(&context, &module, &frame, Value::Reference(Some(local)), 1,)
            .unwrap(),
        Value::Reference(None)
    );
    assert_eq!(context.budget.remaining(), 0);
}
