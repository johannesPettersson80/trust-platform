//! Tests for the per-depth frame payloads (spec 34 MEM-03, MEM-09; census A3, A4).
use super::super::{FrameStack, VmFrame};
use super::*;
use crate::memory::InstanceId;

/// A frame with the given payload, shaped like `push_call_frame` builds one.
fn frame(locals: Vec<Value>, present: Vec<bool>) -> VmFrame {
    VmFrame {
        pou_id: Some(0),
        activation: None,
        return_pc: 0,
        code_start: 0,
        code_end: 1,
        local_ref_start: 0,
        local_ref_count: u32::try_from(locals.len()).expect("local count fits u32"),
        locals,
        parameter_values_present: present,
        runtime_instance: Option::<InstanceId>::None,
        instance_owner: None,
    }
}

/// Spec 34 MEM-03 and MEM-09 (census A3, A4): a payload taken at a reserved depth and
/// returned comes back with the same storage, cleared, so a later activation at that
/// depth allocates nothing. Catches a store that drops the payload on return (a fresh
/// vector each take) or returns a shorter one with stale values.
#[test]
fn a_returned_payload_is_the_same_storage_cleared_on_the_next_take() {
    let mut payloads = FramePayloads::default();
    payloads
        .reserve(2, 3)
        .expect("reserve two depths of three slots");
    let (mut locals, mut present) = payloads.take(0, 2);
    assert_eq!(locals.len(), 2, "count");
    assert_eq!(present, alloc::vec![false; 2]);
    locals[0] = Value::Int(7);
    present[0] = true;
    let address = locals.as_ptr();
    let presence_address = present.as_ptr();
    payloads.reclaim(0, locals, present);
    assert!(!payloads.is_out(0));
    let (locals, present) = payloads.take(0, 2);
    // MEM-09: the same reserved storage, emptied of the previous activation's values.
    assert_eq!(locals.as_ptr(), address);
    assert_eq!(present.as_ptr(), presence_address);
    assert_eq!(locals, alloc::vec![Value::Null; 2]);
    assert_eq!(present, alloc::vec![false; 2]);
}

/// Spec 34 MEM-03 and MEM-09 (census A3): a call frame removed by a trap, fault or abort
/// returns the payload it took, so the next activation at that depth reuses its storage.
/// Catches a payload lost when the frames are unwound rather than returned normally (the
/// next call at the depth then allocates).
#[test]
fn a_trap_inside_a_call_returns_the_frame_payload() {
    let mut stack = FrameStack::default();
    stack.reserve_storage(2).expect("room for two frames");
    stack
        .reserve_frame_payloads(2, 2)
        .expect("two reserved payloads");
    // The program activation takes no payload; the nested call frame does.
    stack
        .push(frame(alloc::vec![Value::Int(1)], alloc::vec![false]))
        .expect("program activation");
    stack.reserve_for_push().expect("room for the call");
    let (locals, present) = stack.take_frame_payload(1, 2);
    let address = locals.as_ptr();
    stack
        .push(frame(locals, present))
        .expect("nested call frame");
    // The trap path unwinds the frames through `pop`; the payload is back at its depth.
    let _trapped = stack.pop().expect("the trapped call frame");
    assert!(!stack.frame_payloads().is_out(1));
    stack.reserve_for_push().expect("room for the next call");
    let (locals, _) = stack.take_frame_payload(1, 2);
    assert_eq!(locals.as_ptr(), address, "the next call reuses the payload");
}

/// Spec 34 MEM-03 (census A3): a depth beyond the reservation, or one whose payload is out,
/// still gives an activation a correctly shaped payload, as an un-reserved stack allocates
/// today. Catches a take that returns the wrong length or reuses an out payload.
#[test]
fn a_take_outside_the_reservation_returns_the_requested_shape() {
    let mut payloads = FramePayloads::default();
    payloads.reserve(1, 4).expect("one reserved depth");
    let (locals, present) = payloads.take(3, 2);
    assert_eq!(locals, alloc::vec![Value::Null; 2]);
    assert_eq!(present, alloc::vec![false; 2]);
    // A reserved depth shorter than the largest POU is cut to the activation's count.
    let (locals, present) = payloads.take(0, 1);
    assert_eq!(locals.len(), 1);
    assert_eq!(present.len(), 1);
}

/// Spec 34 MEM-03 (census A3, A4): a frame that never took a payload leaves the depth's
/// reserved payload untouched, so reclaiming onto it must not replace the reservation.
/// Catches a store that lets a non-payload frame consume another activation's payload.
#[test]
fn reclaim_only_returns_a_payload_that_was_taken() {
    let mut payloads = FramePayloads::default();
    payloads.reserve(1, 2).expect("one reserved depth");
    assert!(!payloads.is_out(0));
    payloads.reclaim(0, alloc::vec![Value::Int(9); 2], alloc::vec![true; 2]);
    let (locals, present) = payloads.take(0, 2);
    assert_eq!(locals, alloc::vec![Value::Null; 2]);
    assert_eq!(present, alloc::vec![false; 2]);
}
