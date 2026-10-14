//! Detached debugger projection of the shared engine's admitted call stack.

use super::DebugSnapshot;
use crate::{error::RuntimeError, memory::VariableStorage, value::Duration};
use trust_runtime_core::{
    bytecode::StorageRole,
    vm::{PreparedModule, StatementFrame},
};

impl DebugSnapshot {
    /// Reconstruct named debugger frames on a detached copy, preserving actual
    /// activation identities so references to suspended caller locals resolve.
    /// This never inserts presentation frames into installed execution storage.
    pub(crate) fn from_execution(
        storage: &VariableStorage,
        now: Duration,
        prepared: &PreparedModule,
        active: StatementFrame<'_>,
    ) -> Result<Self, RuntimeError> {
        let mut snapshot = Self {
            storage_abi: prepared.storage_abi(),
            storage: storage.clone(),
            now,
        };
        snapshot.storage.clear_frames();
        for invocation in active.call_stack() {
            let Some(id) = invocation.activation() else {
                continue;
            };
            let pou = invocation
                .pou_id()
                .ok_or(RuntimeError::InvalidExecutionState)?;
            let owner = prepared
                .pou_name(pou)
                .ok_or(RuntimeError::InvalidExecutionState)?;
            // Snapshot references must resolve against the projected named rows,
            // including the active locals which are not stored in the owner map.
            snapshot.storage.release_execution_frame(id);
            snapshot.storage.push_frame(owner);
            let frame = snapshot
                .storage
                .current_frame_mut()
                .ok_or(RuntimeError::InvalidExecutionState)?;
            frame.id = id;
            frame.instance_id = invocation.runtime_instance();
            for declaration in prepared.frame_declarations(pou) {
                if frame.variables.len() != declaration.slot as usize {
                    return Err(RuntimeError::InvalidExecutionState);
                }
                if declaration.role == StorageRole::Scratch {
                    let start = declaration.slot as usize;
                    let end = start
                        .checked_add(declaration.construction_nodes as usize)
                        .ok_or(RuntimeError::Overflow)?;
                    let scratch = invocation
                        .locals()
                        .get(start..end)
                        .ok_or(RuntimeError::InvalidExecutionState)?;
                    for (offset, value) in scratch.iter().enumerate() {
                        // Keep physical offsets intact for references, but omit
                        // compiler scratch names from debugger variable displays.
                        frame
                            .variables
                            .insert(format!("@scratch/{}", start + offset).into(), value.clone());
                    }
                    continue;
                }
                let name = prepared
                    .strings()
                    .get(declaration.name_idx as usize)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                let value = invocation
                    .locals()
                    .get(declaration.slot as usize)
                    .ok_or(RuntimeError::InvalidExecutionState)?;
                frame.variables.insert(name.clone(), value.clone());
                if declaration.role == StorageRole::Return {
                    frame.return_value = Some(value.clone());
                }
            }
            if frame.variables.len() != invocation.locals().len() {
                return Err(RuntimeError::InvalidExecutionState);
            }
        }
        Ok(snapshot)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::value::{DateTimeValue, Value};
    use std::sync::{Arc, Mutex};
    use trust_runtime_core::vm::{ExecutionServices, PreparationLimits};

    // The board fixture's leaf ends immediately after its return assignment.
    // A pre-instruction observer cannot see that final store after it commits.
    // This separate fixture has a meaningful second statement so the snapshot
    // observes the committed return value while the four real frames remain live.
    const PROJECTION_SOURCE: &str = r#"
VAR_GLOBAL probe_depth : INT := INT#1; END_VAR
FUNCTION ProbeLeaf : INT
VAR echo : INT; END_VAR
ProbeLeaf := INT#1;
echo := ProbeLeaf;
END_FUNCTION
FUNCTION ProbeMiddle : INT
ProbeMiddle := ProbeLeaf() + INT#1;
END_FUNCTION
FUNCTION ProbeOuter : INT
ProbeOuter := ProbeMiddle() + INT#1;
END_FUNCTION
PROGRAM Gpio
VAR depth_result : INT; END_VAR
CASE probe_depth OF
    2: depth_result := ProbeLeaf() + INT#1;
    3: depth_result := ProbeMiddle() + INT#1;
    4: depth_result := ProbeOuter() + INT#1;
ELSE depth_result := INT#1;
END_CASE;
END_PROGRAM
CONFIGURATION Projection
PROGRAM Plant : Gpio;
END_CONFIGURATION
"#;

    struct Observer {
        prepared: Arc<PreparedModule>,
        deepest: Arc<Mutex<Option<DebugSnapshot>>>,
    }
    impl ExecutionServices for Observer {
        fn debug_active(&self) -> bool {
            true
        }
        fn deadline_exceeded(&self) -> bool {
            false
        }
        fn has_wall_clock(&self) -> bool {
            false
        }
        fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
            Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
        }
        fn on_statement_with_frame(
            &self,
            _: u32,
            _: usize,
            _: u32,
            storage: &VariableStorage,
            now: Duration,
            frame: StatementFrame<'_>,
        ) {
            // Automatic-local initialization temporarily adds a private result
            // frame. Observe the ordinary body after its return store commits,
            // while all four actual POU invocations are still live.
            if frame.call_stack().count() == 4 && frame.locals().first() == Some(&Value::Int(1)) {
                let live_locals = frame.locals().to_vec();
                let snapshot =
                    DebugSnapshot::from_execution(storage, now, &self.prepared, frame).unwrap();
                assert_eq!(frame.locals(), live_locals.as_slice());
                assert_eq!(
                    storage.frames().len(),
                    0,
                    "owner retains only real VM frames"
                );
                assert_eq!(storage.execution_frame_count(), 4);
                *self.deepest.lock().unwrap() = Some(snapshot);
            }
        }
    }

    #[test]
    fn real_vm_call_stack_projects_named_locals_without_mutating_controller() {
        let saved = crate::harness::CompileSession::from_source(PROJECTION_SOURCE)
            .build_bytecode_bytes()
            .unwrap();
        let prepared =
            Arc::new(PreparedModule::from_bytes(&saved, PreparationLimits::default()).unwrap());
        let deepest = Arc::new(Mutex::new(None));
        let mut state = Arc::clone(&prepared)
            .instantiate_owned_with_services(
                0,
                Observer {
                    prepared: Arc::clone(&prepared),
                    deepest: Arc::clone(&deepest),
                },
            )
            .unwrap();
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        let mut snapshot = deepest
            .lock()
            .unwrap()
            .take()
            .expect("four actual invocations");
        assert_eq!(snapshot.now, Duration::from_millis(10));
        let names = snapshot
            .storage
            .frames()
            .iter()
            .map(|frame| frame.owner.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Gpio", "ProbeOuter", "ProbeMiddle", "ProbeLeaf"]);
        let leaf = snapshot.storage.current_frame().unwrap();
        assert_eq!(leaf.variables.get("ProbeLeaf"), Some(&Value::Int(1)));
        let leaf_id = leaf.id;
        let leaf_offset = leaf.variables.get_index_of("ProbeLeaf").unwrap();
        let reference = crate::value::ValueRef {
            location: crate::memory::MemoryLocation::Local(leaf_id),
            offset: leaf_offset,
            path: Vec::new(),
        };
        assert_eq!(
            snapshot.storage.read_by_ref_ref(&reference),
            Some(&Value::Int(1))
        );
        snapshot
            .storage
            .with_frame(leaf_id, |storage| {
                assert!(storage.set_local("ProbeLeaf", Value::Int(99)));
            })
            .unwrap();
        assert_eq!(
            snapshot.storage.read_by_ref_ref(&reference),
            Some(&Value::Int(99))
        );
        assert_eq!(state.storage().execution_frame_count(), 0);
        assert!(state.storage().frames().is_empty());
        let Some(Value::Instance(root)) = state.storage().get_global("Plant") else {
            panic!("program root");
        };
        assert_eq!(
            state.storage().get_instance_var(*root, "depth_result"),
            Some(&Value::Int(4))
        );
    }
}
