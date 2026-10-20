//! Flat references (specification 34 MEM-12, §6.3 "Flat references"; C1-11d).
//!
//! The corpus program `references` (`references/src/main.st`) declares the globals `ticks` (a DINT at
//! byte 0), `store` (four DINTs, bytes 4 to 20), `reading` (a `Sample`: the DINT `amount` at 0 and the
//! BOOL `valid` at 4, 8 bytes, at byte 20) and `target` (a DINT at byte 28), each at its declared
//! size and alignment in the persistent block.

use super::*;
use crate::value::RefPath;

/// The path-form reference to the global `name` of `state`, with `path`.
fn global(state: &EngineState<'_>, name: &str, path: RefPath) -> ValueRef {
    ValueRef {
        path,
        ..StoreAccess::ref_for_global(state, name).expect("the global exists")
    }
}

/// Spec 34 MEM-12: a reference to a whole writable target in the persistent block is the block and
/// byte offset of its target, its type and its generation, with no path; it keeps the element count
/// of the path it was taken through, and it is named again by that variable and path. Catches an
/// element offset counted from the wrong bound, a member offset from the wrong member, a flat
/// reference charged as a path of no elements, and an inverse that names another variable.
#[test]
fn a_reference_to_a_persistent_target_is_its_byte_offset() {
    let prepared = prepare(corpus("references"));
    let mut data = build(&prepared).expect("the cell store constructs");
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let dint = |reference: &ValueRef| match reference.location {
        MemoryLocation::Flat { type_id, .. } => type_id,
        other => panic!("{other:?} is not flat"),
    };

    // store[3]: byte 4 + (3 - 1) x 4 = 12; one segment and one subscript.
    let element = global(
        &state,
        "store",
        alloc::vec![RefSegment::Index(alloc::vec![3])],
    );
    let flat = state.flatten_reference(element.clone());
    assert!(flat.path.is_empty());
    assert_eq!(flat.offset, 12);
    assert_eq!(flat.location.flat_elements(), 2);
    assert_eq!(state.path_form(&flat), element);

    // reading.amount: byte 20 + 0; one segment.
    let member = global(
        &state,
        "reading",
        alloc::vec![RefSegment::Field(SmolStr::new("amount"))],
    );
    let flat_member = state.flatten_reference(member.clone());
    assert_eq!(flat_member.offset, 20);
    assert_eq!(flat_member.location.flat_elements(), 1);
    assert_eq!(state.path_form(&flat_member), member);

    // target: byte 28, the whole variable; both are DINTs.
    let whole = global(&state, "target", RefPath::new());
    let flat_whole = state.flatten_reference(whole.clone());
    assert_eq!(flat_whole.offset, 28);
    assert_eq!(flat_whole.location.flat_elements(), 0);
    assert_eq!(dint(&flat_whole), dint(&flat));
    assert_eq!(state.path_form(&flat_whole), whole);
}

/// Spec 34 §6.3: a reference to a declaration that cannot be written keeps its root and path, so a
/// write through it is still refused by its declaration. Catches a constant made flat, whose write
/// would no longer find the declaration that refuses it.
#[test]
fn a_reference_to_a_constant_keeps_its_root_and_path() {
    let prepared = prepare(corpus("initializers"));
    let mut data = build(&prepared).expect("the cell store constructs");
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let constant = global(&state, "capacity", RefPath::new());
    assert_eq!(state.flatten_reference(constant.clone()), constant);
}
