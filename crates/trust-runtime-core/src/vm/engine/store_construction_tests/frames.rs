//! Frames on the activation stack (specification 34 MEM-12, §6.3 "Activation stack"; C1-11).
//!
//! The corpus program `calls` (`calls/src/main.st`) declares four functions. Their frames, each
//! variable at its declared size and alignment and the frame rounded up to 8 bytes:
//! `Scale` holds `Scale`, `value` and `factor`, three DINTs at 0, 4 and 8, so 16 bytes; `Digits`
//! holds the BOOL `Digits` at 0 and the DINTs `value`, `low` and `high` at 4, 8 and 12, so 16;
//! `Twice` holds two DINTs, 8; `Bump` holds two DINTs, its VAR_IN_OUT copied in and back, 8.

use super::*;
use crate::memory::FrameId;
use crate::vm::VmFrame;

/// The frame of a new activation of `pou` that holds `locals`, as the dispatcher pushes it.
fn frame_of(prepared: &PreparedModule, pou: u32, locals: Vec<Value>) -> VmFrame {
    let entry = prepared.vm.pou(pou).expect("the POU exists");
    VmFrame {
        activation: None,
        pou_id: Some(pou),
        return_pc: 0,
        code_start: entry.code_start,
        code_end: entry.code_end,
        local_ref_start: entry.local_ref_start,
        local_ref_count: entry.local_ref_count,
        locals,
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

/// The POU id of the function `name` of `prepared`.
fn function(prepared: &PreparedModule, name: &str) -> u32 {
    (0..1024)
        .find(|&id| {
            prepared.vm.pou(id).is_some_and(|pou| {
                pou.kind == crate::bytecode::PouKind::Function
                    && pou.name.eq_ignore_ascii_case(name)
            })
        })
        .expect("the function exists")
}

/// Spec 34 §6.3: the activation stack reserves one frame of the largest POU for each call level
/// construction reserves. In `calls` the largest frames are `Scale`'s and `Digits`' 16 bytes.
/// Catches a frame sized in 16-byte cells per variable (48 bytes for `Scale`) and a reservation
/// that ignores the call levels.
#[test]
fn the_activation_stack_reserves_a_largest_frame_for_each_call_level() {
    let prepared = prepare(corpus("calls"));
    for name in ["Scale", "Digits"] {
        let range = prepared
            .local_range(function(&prepared, name))
            .expect("the function has a frame");
        assert_eq!(range.bytes, 16, "{name}");
    }
    for name in ["Twice", "Bump"] {
        let range = prepared
            .local_range(function(&prepared, name))
            .expect("the function has a frame");
        assert_eq!(range.bytes, 8, "{name}");
    }
    let levels = u32::try_from(prepared.reserved_call_levels()).unwrap();
    // Calls -> Twice -> Scale is three levels deep.
    assert!(levels >= 3, "{levels} call levels");
    assert_eq!(prepared.activation_reservation().unwrap(), levels * 16);
}

/// Spec 34 MEM-12 and §6.3: a frame is placed on top of its caller's, its inputs move into its
/// bytes, a slot no value was given for starts at zero, and ending the activation takes the frame
/// off the stack. Catches a frame that keeps its inputs as values of the frame, one placed over its
/// caller's bytes, and one that stays on the stack after its activation ends.
#[test]
fn a_frame_lies_on_top_of_its_caller_and_holds_its_inputs() {
    let prepared = prepare(corpus("calls"));
    let mut data = build(&prepared).expect("the cell store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);

    let mut outer = frame_of(
        &prepared,
        function(&prepared, "Scale"),
        vec![Value::Null, Value::DInt(5), Value::DInt(3)],
    );
    state.place_frame(&mut outer, FrameId(7)).unwrap();
    outer.activation = Some(FrameId(7));
    assert_eq!(outer.locals, [Value::Null, Value::Null, Value::Null]);
    assert_eq!(
        state.read_frame_slot(&outer, 1).unwrap().unwrap(),
        Value::DInt(5)
    );
    assert_eq!(
        state.read_frame_slot(&outer, 2).unwrap().unwrap(),
        Value::DInt(3)
    );
    assert_eq!(
        state.read_frame_slot(&outer, 0).unwrap().unwrap(),
        Value::DInt(0)
    );

    let mut inner = frame_of(
        &prepared,
        function(&prepared, "Twice"),
        vec![Value::Null, Value::DInt(9)],
    );
    state.place_frame(&mut inner, FrameId(8)).unwrap();
    inner.activation = Some(FrameId(8));
    let block = state
        .frame_block(&inner)
        .expect("the frame is on the stack");
    assert_eq!(
        block.range,
        ByteRange {
            region: LayoutRegion::Activation,
            start: 16,
            bytes: 8,
        }
    );
    assert_eq!(
        state.read_frame_slot(&inner, 1).unwrap().unwrap(),
        Value::DInt(9)
    );
    assert_eq!(
        state.read_frame_slot(&outer, 1).unwrap().unwrap(),
        Value::DInt(5)
    );

    state.release_frame_block(FrameId(8));
    assert!(state.frame_block(&inner).is_none());
    assert_eq!(
        state.read_finished_slot(block, 1).unwrap().unwrap(),
        Value::DInt(9)
    );
    let mut next = frame_of(
        &prepared,
        function(&prepared, "Bump"),
        vec![Value::Null, Value::DInt(1)],
    );
    state.place_frame(&mut next, FrameId(9)).unwrap();
    next.activation = Some(FrameId(9));
    assert_eq!(state.frame_block(&next).unwrap().range.start, 16);
    state.release_frame_block(FrameId(9));
    state.release_frame_block(FrameId(7));
    assert!(state.frame_block(&outer).is_none());
}
