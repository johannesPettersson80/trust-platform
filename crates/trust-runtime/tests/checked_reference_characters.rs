//! Checked owner and cycle reads materialize text elements without exposing storage.
use std::sync::Arc;
use trust_runtime::{bytecode::BytecodeVersion, harness::CompileSession};
use trust_runtime_core::{
    error::RuntimeError,
    memory::VariableStorage,
    value::{Duration, RefSegment, Value, ValueRef},
    vm::{CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule},
};

fn selected(storage: &VariableStorage, field: &str, index: i64) -> ValueRef {
    let mut reference = storage.ref_for_global("Main").unwrap();
    reference.path.extend([
        RefSegment::Field(field.into()),
        RefSegment::Index(vec![index]),
    ]);
    reference
}

struct ReadCharacters {
    samples: usize,
}
impl CycleHooks for ReadCharacters {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        if phase == CyclePhase::BeforeInputSample {
            for (field, expected) in [
                ("narrow", Value::Char(b'B')),
                ("wide", Value::WChar(0x03b2)),
            ] {
                let reference = selected(access.storage(), field, 2);
                assert_eq!(access.read_reference(&reference)?, expected);
                let invalid = selected(access.storage(), field, 3);
                assert_eq!(
                    access.read_reference(&invalid),
                    Err(RuntimeError::NullReference)
                );
            }
            self.samples += 1;
        }
        Ok(())
    }
}

#[test]
fn saved_application_owner_and_cycle_ports_read_char_and_wchar_elements() {
    let bytes = CompileSession::from_source(
        r#"
PROGRAM Main
VAR
    narrow : STRING[8] := 'AB';
    wide : WSTRING[8] := "αβ";
END_VAR
END_PROGRAM
"#,
    )
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
    .unwrap()
    .encode()
    .unwrap();
    let prepared =
        Arc::new(PreparedModule::from_bytes(&bytes, PreparationLimits::default()).unwrap());
    let mut state = prepared.instantiate_owned(0).unwrap();
    let readiness: Vec<_> = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.overrun_count, task.last_single))
        .collect();
    let images = state.outputs().to_vec();
    for (field, expected) in [
        ("narrow", Value::Char(b'B')),
        ("wide", Value::WChar(0x03b2)),
    ] {
        let reference = selected(state.storage(), field, 2);
        assert_eq!(state.read_reference(&reference).unwrap(), expected);
        let invalid = selected(state.storage(), field, 3);
        assert_eq!(
            state.read_reference(&invalid),
            Err(RuntimeError::NullReference)
        );
        let mut invalid = reference.clone();
        invalid
            .path
            .push(RefSegment::Field("not_a_character_field".into()));
        assert_eq!(
            state.read_reference(&invalid),
            Err(RuntimeError::NullReference)
        );
    }
    let after: Vec<_> = state
        .task_states()
        .iter()
        .map(|task| (task.last_run, task.overrun_count, task.last_single))
        .collect();
    assert_eq!(after, readiness);
    assert_eq!(state.outputs(), images);
    assert!(state.fault().is_none());
    let mut hooks = ReadCharacters { samples: 0 };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .unwrap();
    assert_eq!(hooks.samples, 1);
    assert!(state.fault().is_none());
}
