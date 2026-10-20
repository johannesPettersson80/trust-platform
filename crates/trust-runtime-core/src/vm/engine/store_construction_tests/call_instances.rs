//! Per-call instances on the activation stack (specification 34 MEM-12, §6.3 "Activation stack";
//! C1-11c).
//!
//! The corpus program `instances` (`instances/src/main.st`) declares the class `Accumulator`, whose
//! own variable is the DINT `sum := 0`, and `Window EXTENDS Accumulator`, whose own variable is the
//! DINT `width := 3`; each level's own variables are rounded up to 8 bytes, so 8 bytes each. The
//! function `Window3` holds the DINTs `Window3` at 0 and `seed` at 4, the 8-byte instance leaf
//! `scratch` at 8 and the DINT `index` at 16, 20 bytes rounded up to 24. A call of it creates
//! `scratch`, a `Window` with its `Accumulator` part: two instances of 8 bytes each.

use super::*;
use crate::bytecode::PouKind;
use crate::vm::context::ExecutionContext;
use crate::vm::VmFrame;

/// The POU id of the POU `name` of kind `kind` in `prepared`.
fn pou_named(prepared: &PreparedModule, kind: PouKind, name: &str) -> u32 {
    (0..1024)
        .find(|&id| {
            prepared
                .vm
                .pou(id)
                .is_some_and(|pou| pou.kind == kind && pou.name.eq_ignore_ascii_case(name))
        })
        .expect("the POU exists")
}

/// The frame of a new activation of `pou`, its locals empty, as the dispatcher pushes it.
fn new_frame(prepared: &PreparedModule, pou: u32) -> VmFrame {
    let entry = prepared.vm.pou(pou).expect("the POU exists");
    VmFrame {
        activation: None,
        pou_id: Some(pou),
        return_pc: 0,
        code_start: entry.code_start,
        code_end: entry.code_end,
        local_ref_start: entry.local_ref_start,
        local_ref_count: entry.local_ref_count,
        locals: vec![Value::Null; entry.local_ref_count as usize],
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    }
}

/// Spec 34 §6.3: for each call level the stack reserves the largest frame together with the
/// instances its variables hold, and a record for each of those instances. `Window3`'s 24 bytes
/// hold `scratch`, whose `Window` and `Accumulator` parts take 8 bytes each: 40 bytes and two
/// records a level. Catches a reservation that leaves the instances out (24 a level), one that
/// counts the inherited part twice or not at all (48 or 32), and records reserved for the frames
/// only.
#[test]
fn the_activation_stack_reserves_the_instances_a_frame_holds() {
    let prepared = prepare(corpus("instances"));
    let window3 = pou_named(&prepared, PouKind::Function, "Window3");
    assert_eq!(prepared.local_range(window3).unwrap().bytes, 24);
    let levels = prepared.reserved_call_levels();
    // Instances -> Window3 -> Accumulator.Add is three levels deep.
    assert!(levels >= 3, "{levels} call levels");
    assert_eq!(
        prepared.activation_reservation().unwrap(),
        u32::try_from(levels).unwrap() * 40
    );
    assert_eq!(prepared.call_instance_reservation().unwrap(), levels * 2);
}

/// Spec 34 MEM-12 and §6.3: an instance a call creates lies on the stack above the call's frame,
/// each level of its inheritance chain in its own range, its variables start at zero and take
/// their declared constants, and it leaves the stack when its frame ends. Catches an instance
/// created in the `Value` half (no range on the stack), one placed over its frame's bytes, a
/// parent part placed away from its child, a declared constant left unwritten (`width` reading 0),
/// and an instance left on the stack after its frame ends.
#[test]
fn a_call_creates_its_instances_above_its_frame_and_ends_them_with_it() {
    let prepared = prepare(corpus("instances"));
    let mut data = build(&prepared).expect("the cell store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let window3 = pou_named(&prepared, PouKind::Function, "Window3");
    let mut frame = new_frame(&prepared, window3);
    state
        .engineering_operation(|state| state.initialize_frame(&prepared.vm, &mut frame, 1))
        .expect("the frame initializes");
    assert_eq!(state.frame_block(&frame).unwrap().range.start, 0);

    let Value::Instance(window) = state.read_frame_slot(&frame, 2).unwrap().unwrap() else {
        panic!("scratch holds an instance");
    };
    let parent = state
        .instance_parent(window)
        .unwrap()
        .expect("a Window has its Accumulator part");
    let arm = &*state.data.store;
    let on_stack = |start| ByteRange {
        region: LayoutRegion::Activation,
        start,
        bytes: 8,
    };
    assert_eq!(arm.instance_range(window), Some(on_stack(24)));
    assert_eq!(arm.instance_range(parent), Some(on_stack(32)));
    assert_eq!(arm.activation_top(), 40);
    let window_data = state.snapshot_instance(window).unwrap();
    assert_eq!(window_data.variables.get("width"), Some(&Value::DInt(3)));
    let parent_data = state.snapshot_instance(parent).unwrap();
    assert_eq!(parent_data.variables.get("sum"), Some(&Value::DInt(0)));

    state
        .engineering_operation(|state| state.retire_frame(&frame))
        .expect("the frame retires");
    let arm = &*state.data.store;
    assert_eq!(arm.instance_range(window), None);
    assert_eq!(arm.instance_range(parent), None);
    assert_eq!(arm.activation_top(), 0);
}
