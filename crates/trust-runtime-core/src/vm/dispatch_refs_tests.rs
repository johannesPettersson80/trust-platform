use super::*;
use crate::error::RuntimeError;
use crate::memory::{InstanceId, VariableStorage};
use crate::value::{RefPath, ValueRefView};
use crate::vm::{context::ReferenceContext, frames::VmFrame, module::VmRef, VmTrap};
use alloc::{vec, vec::Vec};

struct CandidateContext {
    storage: VariableStorage,
    candidates: [u32; 2],
}

impl CandidateContext {
    fn new() -> Self {
        Self {
            storage: VariableStorage::new(),
            candidates: [0, 1],
        }
    }
}

impl ReferenceContext for CandidateContext {
    fn dynamic_reference_candidates(
        &self,
        _: &VmModule,
        _: ValueRefView<'_>,
    ) -> Result<Option<&[u32]>, RuntimeError> {
        Ok(Some(&self.candidates))
    }

    fn check_reference_write(&self, _: ValueRefView<'_>, _: &Value) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn check_reference_read(&self, _: ValueRefView<'_>) -> Result<(), RuntimeError> {
        Ok(())
    }

    fn initializer_reference<'a>(
        &self,
        _: u32,
        _: &'a [RefSegment],
    ) -> Result<ValueRefView<'a>, VmTrap> {
        Err(VmTrap::NullReference)
    }

    fn resolve_instance_owner(&self, encoded: u32, _: &VmFrame) -> Result<InstanceId, VmTrap> {
        match encoded {
            10 => Err(VmTrap::Runtime(RuntimeError::NullReference)),
            20 => Ok(InstanceId(44)),
            _ => Err(VmTrap::NullReference),
        }
    }

    fn storage(&self) -> &VariableStorage {
        &self.storage
    }

    fn storage_mut(&mut self) -> &mut VariableStorage {
        &mut self.storage
    }

    fn write_typed_storage(
        &mut self,
        _: MemoryLocation,
        _: usize,
        _: &[RefSegment],
        _: Value,
    ) -> Result<bool, RuntimeError> {
        Ok(false)
    }

    fn write_typed_local(&self, _: &mut Value, _: &[RefSegment], _: Value) -> bool {
        false
    }
}

fn candidate_module() -> VmModule {
    let bytes = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
    ));
    let prepared =
        super::super::PreparedModule::from_bytes(bytes, super::super::PreparationLimits::default())
            .unwrap();
    let mut module = prepared.vm;
    module.refs = vec![
        VmRef::Instance {
            owner_instance_id: 10,
            offset: 0,
            path: RefPath::new(),
        },
        VmRef::Instance {
            owner_instance_id: 20,
            offset: 0,
            path: RefPath::new(),
        },
    ];
    module.ref_types = vec![Some(0), Some(0)];
    module
}

#[test]
fn stale_owner_candidate_is_skipped_without_lookup_charges() {
    let module = candidate_module();
    let frame = VmFrame {
        activation: None,
        pou_id: None,
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };
    let reference = ValueRef {
        location: MemoryLocation::Instance(InstanceId(44)),
        offset: 0,
        path: RefPath::new(),
    };
    let context = CandidateContext::new();

    // TIME-08: owner and candidate lookups are bookkeeping. The stale owner is skipped and the
    // live candidate still resolves to the same type.
    assert_eq!(
        dynamic_ref_type(&context, &module, &frame, &reference).unwrap(),
        Some(0)
    );
}
