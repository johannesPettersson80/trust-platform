use super::*;
use crate::value::{ArrayValue, StructValue};
use alloc::{boxed::Box, sync::Arc, vec};

#[test]
fn returned_locals_reset_in_place_and_release_nested_payloads() {
    let mut frames = FrameStack::default();
    frames.try_reserve_frame_payload(3).unwrap();
    let (mut locals, presence) = frames.take_reserved_frame_payload(3).unwrap();
    frames.reserved_parameter_presence = presence;
    let pointer = locals.as_ptr();
    let capacity = locals.capacity();
    let shared = Arc::new(StructValue::from_canonical_parts(
        "Record".into(),
        crate::collections::OrderedMap::default(),
    ));
    locals[0] = Value::Struct(Arc::clone(&shared));
    locals[1] = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Struct(Arc::clone(&shared))],
        vec![(0, 0)],
    )));
    locals[2] = Value::String("text longer than the inline string capacity".into());
    assert_eq!(Arc::strong_count(&shared), 3);

    frames.recycle_returned_locals(locals).unwrap();
    assert_eq!(Arc::strong_count(&shared), 1);
    assert!(frames.has_reserved_frame_payload());
    let (mut locals, _) = frames.take_reserved_frame_payload(3).unwrap();
    assert_eq!(locals.as_ptr(), pointer);
    assert_eq!(locals.capacity(), capacity);
    assert_eq!(locals, vec![Value::Null; 3]);
    locals[1] = Value::Int(41);
    frames.recycle_returned_locals(locals).unwrap();
    assert_eq!(frames.reserved_locals.as_ptr(), pointer);
    assert_eq!(frames.reserved_locals, vec![Value::Null; 3]);
}

#[test]
fn returned_locals_refuse_wrong_lengths_and_occupied_reservation() {
    let mut frames = FrameStack::default();
    frames.try_reserve_frame_payload(2).unwrap();
    let pointer = frames.reserved_locals.as_ptr();
    assert_eq!(
        frames.recycle_returned_locals(vec![Value::Int(1), Value::Int(2)]),
        Err(crate::error::RuntimeError::InvalidExecutionState)
    );
    assert_eq!(frames.reserved_locals.as_ptr(), pointer);
    assert_eq!(frames.reserved_locals, vec![Value::Null; 2]);
    let (locals, _) = frames.take_reserved_frame_payload(2).unwrap();
    assert_eq!(
        frames.recycle_returned_locals(vec![Value::Int(1)]),
        Err(crate::error::RuntimeError::InvalidExecutionState)
    );
    assert!(frames.reserved_locals.is_empty());
    frames.recycle_returned_locals(locals).unwrap();
    assert_eq!(frames.reserved_locals.as_ptr(), pointer);
}
