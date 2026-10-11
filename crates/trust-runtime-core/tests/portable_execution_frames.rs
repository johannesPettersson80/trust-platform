//! Source-free local ownership and reference-lifetime regressions.
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::memory::{MemoryLocation, VariableStorage};
use trust_runtime_core::value::{RefPath, Value, ValueRef};

#[test]
fn suspended_ancestor_reference_mutates_the_original_local_vector() {
    let mut storage = VariableStorage::new();
    let caller = storage.reserve_execution_frame().unwrap();
    let callee = storage.reserve_execution_frame().unwrap();
    let mut caller_locals = vec![Value::Int(3)];
    let mut callee_locals = vec![Value::Int(7)];
    storage
        .suspend_execution_frame(caller, &mut caller_locals)
        .unwrap();
    storage
        .suspend_execution_frame(callee, &mut callee_locals)
        .unwrap();
    let reference = ValueRef {
        location: MemoryLocation::Local(caller),
        offset: 0,
        path: RefPath::new(),
    };
    assert!(storage.write_by_ref_ref(&reference, Value::Int(11)));
    storage
        .resume_execution_frame(callee, &mut callee_locals)
        .unwrap();
    storage
        .resume_execution_frame(caller, &mut caller_locals)
        .unwrap();
    assert_eq!(caller_locals, vec![Value::Int(11)]);
    assert_eq!(callee_locals, vec![Value::Int(7)]);
    assert!(storage.release_execution_frame(caller));
    assert!(storage.read_by_ref_ref(&reference).is_none());
    assert!(!storage.write_by_ref_ref(&reference, Value::Int(99)));
    assert_ne!(storage.reserve_execution_frame().unwrap(), caller);
}

#[test]
fn invalid_ownership_transfers_preserve_both_local_vectors() {
    let mut storage = VariableStorage::new();
    let id = storage.reserve_execution_frame().unwrap();
    let mut locals = vec![Value::Int(3)];
    storage.suspend_execution_frame(id, &mut locals).unwrap();
    let mut unrelated = vec![Value::Int(7)];
    assert_eq!(
        storage.suspend_execution_frame(id, &mut unrelated),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        storage.resume_execution_frame(id, &mut unrelated),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(unrelated, vec![Value::Int(7)]);
    storage.resume_execution_frame(id, &mut locals).unwrap();
    assert_eq!(locals, vec![Value::Int(3)]);
    assert_eq!(
        storage.resume_execution_frame(id, &mut Vec::new()),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert!(storage.release_execution_frame(id));
    assert_eq!(
        storage.suspend_execution_frame(id, &mut locals),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(
        storage.resume_execution_frame(id, &mut Vec::new()),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(locals, vec![Value::Int(3)]);
}
