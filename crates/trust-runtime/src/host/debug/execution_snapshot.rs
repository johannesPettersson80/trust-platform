//! Detached debugger projection of the shared engine's admitted call stack.

use super::{hook::DebugRuntimeContext, DebugSnapshot};
use crate::{
    error::RuntimeError,
    memory::{FrameId, VariableStorage},
    stdlib::StandardLibrary,
    value::{DateTimeProfile, Duration},
};
use trust_hir::types::TypeRegistry;
use trust_runtime_core::{
    bytecode::StorageRole,
    vm::{PreparedModule, StatementFrame},
};

/// The live storage of one statement of a hosted scan, read where the scan runs it.
///
/// Nothing is copied when the statement is observed. The detached projection that a stop, a
/// breakpoint condition or a logpoint needs is built on first use, and it is the only copy the
/// statement makes (specification 11 section 6.13.1).
pub(crate) struct StatementProjection<'a> {
    storage: &'a VariableStorage,
    now: Duration,
    frame: StatementFrame<'a>,
    prepared: &'a PreparedModule,
    registry: &'a TypeRegistry,
    stdlib: &'a StandardLibrary,
    profile: DateTimeProfile,
    detached: Option<DebugSnapshot>,
    copies: u64,
    published: bool,
}

impl<'a> StatementProjection<'a> {
    /// Observe the scan's storage and call stack at one statement boundary, without copying them.
    pub(crate) fn new(
        storage: &'a VariableStorage,
        now: Duration,
        frame: StatementFrame<'a>,
        prepared: &'a PreparedModule,
        registry: &'a TypeRegistry,
        stdlib: &'a StandardLibrary,
        profile: DateTimeProfile,
    ) -> Self {
        Self {
            storage,
            now,
            frame,
            prepared,
            registry,
            stdlib,
            profile,
            detached: None,
            copies: 0,
            published: false,
        }
    }

    /// The storage copies this statement has made: at most one, when a projection was built.
    pub(crate) fn copies(&self) -> u64 {
        self.copies
    }

    /// Whether this statement's projection has already been published to a stop.
    pub(crate) fn is_published(&self) -> bool {
        self.published
    }

    /// The identities of the invocations on the call stack, the active one last.
    pub(crate) fn active_frames(&self) -> impl Iterator<Item = FrameId> + '_ {
        self.frame
            .call_stack()
            .filter_map(|invocation| invocation.activation())
    }

    /// Build the detached projection once, and return it for reading or moving.
    fn detached(&mut self) -> Result<&mut DebugSnapshot, RuntimeError> {
        if self.detached.is_none() {
            let snapshot =
                DebugSnapshot::from_execution(self.storage, self.now, self.prepared, self.frame)?;
            self.copies = self.copies.saturating_add(1);
            self.detached = Some(snapshot);
        }
        self.detached
            .as_mut()
            .ok_or(RuntimeError::InvalidExecutionState)
    }

    /// Run `f` with the debugger's evaluation context over the detached projection.
    pub(crate) fn with_context<R>(
        &mut self,
        f: impl FnOnce(&mut DebugRuntimeContext<'_>) -> R,
    ) -> Result<R, RuntimeError> {
        let storage_abi = self.prepared.storage_abi();
        let (registry, stdlib, profile, now) = (self.registry, self.stdlib, self.profile, self.now);
        let snapshot = self.detached()?;
        let current_instance = snapshot
            .storage
            .current_frame()
            .and_then(|frame| frame.instance_id);
        let mut ctx = DebugRuntimeContext {
            storage_abi,
            storage: &mut snapshot.storage,
            registry,
            stdlib: Some(stdlib),
            profile,
            current_instance,
            now,
        };
        Ok(f(&mut ctx))
    }

    /// Move the detached projection out for a stop, and mark the statement as published.
    pub(crate) fn take(&mut self) -> Result<DebugSnapshot, RuntimeError> {
        self.detached()?;
        self.published = true;
        self.detached
            .take()
            .ok_or(RuntimeError::InvalidExecutionState)
    }
}

impl DebugSnapshot {
    /// Reconstruct named debugger frames on a detached copy, preserving actual
    /// activation identities so references to suspended caller locals resolve.
    /// This never inserts presentation frames into installed execution storage.
    /// On the flat store the copy and the locals are read from where they lie
    /// (specification 34 §6.3).
    pub(crate) fn from_execution(
        storage: &VariableStorage,
        now: Duration,
        prepared: &PreparedModule,
        active: StatementFrame<'_>,
    ) -> Result<Self, RuntimeError> {
        let mut snapshot = Self {
            storage_abi: prepared.storage_abi(),
            storage: active.storage_copy(storage)?,
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
                let value = invocation.local(declaration.slot as usize)?;
                if declaration.role == StorageRole::Return {
                    frame.return_value = Some(value.clone());
                }
                frame.variables.insert(name.into(), value);
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

    /// Keeps the projection of the last instruction at which the four invocations are live and the
    /// leaf has its result, on the flat store, where the frames' own entries for typed locals are
    /// `Null`, so the condition reads the result with `StatementFrame::local`.
    struct FlatObserver {
        prepared: Arc<PreparedModule>,
        deepest: Arc<Mutex<Option<DebugSnapshot>>>,
    }
    impl ExecutionServices for FlatObserver {
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
            if frame.call_stack().count() == 4 && matches!(frame.local(0), Ok(Value::Int(1))) {
                let snapshot =
                    DebugSnapshot::from_execution(storage, now, &self.prepared, frame).unwrap();
                *self.deepest.lock().unwrap() = Some(snapshot);
            }
        }
    }

    #[test]
    fn the_flat_store_projects_the_call_stack_and_the_variables_from_where_they_lie() {
        // Specification 34 §6.3 (C1-17) and specification 11 §6.13.1: on the flat store the typed
        // locals of each frame lie on the activation stack, and the globals and the program's
        // instance in the persistent block, so the debugger's projection reads them there. With
        // probe_depth 4 the program calls ProbeOuter, ProbeMiddle and ProbeLeaf; at the leaf's
        // second statement the four invocations are live and the leaf has assigned its result 1.
        // The global probe_depth holds the 4 written before the scan, and the program's
        // depth_result still holds its initial 0, which the scan assigns only after the calls
        // return. A projection that read the frames' own entries would show `Null` for the leaf's
        // result and never meet the condition; one that copied only the value half would hold no
        // probe_depth and no program instance.
        let saved = crate::harness::CompileSession::from_source(PROJECTION_SOURCE)
            .build_bytecode_bytes()
            .unwrap();
        let prepared =
            Arc::new(PreparedModule::from_bytes(&saved, PreparationLimits::default()).unwrap());
        let observer = FlatObserver {
            prepared: Arc::clone(&prepared),
            deepest: Arc::new(Mutex::new(None)),
        };
        let mut state = prepared
            .instantiate_on_cells_with_services(0, &observer)
            .unwrap();
        state.write_global("probe_depth", Value::Int(4)).unwrap();
        state.execute_cycle(Duration::from_millis(10)).unwrap();
        let snapshot = observer
            .deepest
            .lock()
            .unwrap()
            .take()
            .expect("four actual invocations");
        let names = snapshot
            .storage
            .frames()
            .iter()
            .map(|frame| frame.owner.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, ["Gpio", "ProbeOuter", "ProbeMiddle", "ProbeLeaf"]);
        let leaf = snapshot.storage.current_frame().unwrap();
        assert_eq!(leaf.variables.get("ProbeLeaf"), Some(&Value::Int(1)));
        assert_eq!(leaf.return_value, Some(Value::Int(1)));
        let leaf_offset = leaf.variables.get_index_of("ProbeLeaf").unwrap();
        let reference = crate::value::ValueRef {
            location: crate::memory::MemoryLocation::Local(leaf.id),
            offset: u32::try_from(leaf_offset).unwrap(),
            generation: snapshot.storage.generation(),
            path: Vec::new(),
        };
        assert_eq!(
            snapshot.storage.read_by_ref_ref(&reference),
            Some(&Value::Int(1))
        );
        assert_eq!(
            snapshot.storage.get_global("probe_depth"),
            Some(&Value::Int(4))
        );
        let Some(Value::Instance(root)) = snapshot.storage.get_global("Plant") else {
            panic!("program root");
        };
        assert_eq!(
            snapshot.storage.get_instance_var(*root, "depth_result"),
            Some(&Value::Int(0))
        );
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
            offset: u32::try_from(leaf_offset).unwrap(),
            generation: snapshot.storage.generation(),
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
