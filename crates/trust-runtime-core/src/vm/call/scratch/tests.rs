//! Unit tests of the reserved call scratch (spec 34 MEM-09, §6.3).
use super::*;

/// Sizes a fresh vector would have to allocate for: a call with four arguments, a POU with three
/// parameters and five local slots.
const SIZES: CallScratchSizes = CallScratchSizes {
    args: 4,
    params: 3,
    slots: 5,
};

/// Spec 34 MEM-09 (§6.3 call scratch): a reserved scratch serves every kind of binding vector
/// from the storage construction reserved, and a returned vector keeps that storage. Catches a
/// take that allocates instead of taking the reserved vector, a kind sized below the call it
/// must serve, and a return that drops the storage.
#[test]
fn reserved_call_scratch_serves_every_kind_from_its_reserved_storage() {
    let mut scratch = CallScratch::default();
    scratch.reserve(2, SIZES).unwrap();

    let args = scratch.take_args();
    assert_eq!(args.capacity(), SIZES.args, "argument vector");
    let flags = scratch.take_flags(SIZES.slots);
    assert_eq!(flags.len(), SIZES.slots, "flag vector length");
    assert_eq!(
        flags.capacity(),
        SIZES.slots.max(SIZES.args),
        "flag vector capacity"
    );
    let function_outputs = scratch.take_function_outputs();
    assert_eq!(
        function_outputs.capacity(),
        SIZES.params,
        "function outputs"
    );
    let fb_outputs = scratch.take_fb_outputs();
    assert_eq!(
        fb_outputs.capacity(),
        SIZES.params,
        "function-block outputs"
    );
    let prepared_targets = scratch.take_prepared_targets();
    assert_eq!(
        prepared_targets.capacity(),
        SIZES.params,
        "prepared targets"
    );
    let prepared_outputs = scratch.take_prepared_outputs();
    assert_eq!(
        prepared_outputs.capacity(),
        SIZES.params,
        "prepared outputs"
    );
    let stdlib_values = scratch.take_stdlib_values();
    assert_eq!(
        stdlib_values.capacity(),
        SIZES.args,
        "standard-library values"
    );

    scratch.recycle_args(args);
    scratch.recycle_flags(flags);
    scratch.recycle_function_outputs(function_outputs);
    scratch.recycle_fb_outputs(fb_outputs);
    scratch.recycle_prepared_targets(prepared_targets);
    scratch.recycle_prepared_outputs(prepared_outputs);
    scratch.recycle_stdlib_values(stdlib_values);
    assert_eq!(scratch.held_counts(), [2; KIND_COUNT]);
}

/// Spec 34 MEM-09 (§6.3 call scratch): two calls active at once take different reserved vectors,
/// so a nested call uses its own depth's scratch. Catches a pool that hands the same vector to a
/// nested call.
#[test]
fn nested_calls_take_their_own_depths_scratch_storage() {
    let mut scratch = CallScratch::default();
    scratch.reserve(2, SIZES).unwrap();

    let outer = scratch.take_function_outputs();
    let inner = scratch.take_function_outputs();
    assert_ne!(
        outer.as_ptr(),
        inner.as_ptr(),
        "a nested call must not share the outer scratch"
    );
    assert_eq!(scratch.most_outstanding()[KIND_FUNCTION_OUTPUTS], 2);

    scratch.recycle_function_outputs(outer);
    scratch.recycle_function_outputs(inner);
    assert_eq!(scratch.held_counts()[KIND_FUNCTION_OUTPUTS], 2);
}

/// Spec 34 MEM-09 (§6.3 call scratch): a depth beyond the reservation falls back to a fresh
/// vector, and returning it drops it, so the pool never holds more than its reservation. Catches
/// a fallback that takes a reserved vector or a return that grows the pool.
#[test]
fn a_depth_beyond_the_reservation_allocates_fresh_and_a_return_never_grows_the_pool() {
    let mut scratch = CallScratch::default();
    scratch.reserve(1, SIZES).unwrap();

    let reserved = scratch.take_args();
    let fresh = scratch.take_args();
    assert_eq!(
        reserved.capacity(),
        SIZES.args,
        "the reserved depth's vector"
    );
    assert_eq!(fresh.capacity(), 0, "a further depth takes a fresh vector");

    // Calls return their vectors in the order they took them: the fallback goes back first.
    scratch.recycle_args(fresh);
    scratch.recycle_args(reserved);
    assert_eq!(
        scratch.held_counts()[KIND_ARGS],
        1,
        "the fallback beyond the reservation is dropped, not pooled"
    );
    assert_eq!(
        scratch.take_args().capacity(),
        SIZES.args,
        "the reserved storage is reused"
    );
}

/// Spec 34 MEM-09 (§6.3 call scratch): the reservation is charged for the depth table and every
/// reserved element, so construction admits it (TIME-09). Catches a charge that omits a kind.
#[test]
fn reservation_bytes_covers_the_depth_table_and_every_kind() {
    let bytes = CallScratch::reservation_bytes(2, SIZES).expect("no overflow");
    let mut scratch = CallScratch::default();
    scratch.reserve(2, SIZES).unwrap();
    assert!(bytes > 0);
    // Every reserved vector holds its full size in the reserved storage: 4 arguments, 5 flags,
    // 3 of each output kind, per depth.
    let mut per_depth = SIZES.args * core::mem::size_of::<VmNativeArg>();
    per_depth += SIZES.flags() * core::mem::size_of::<bool>();
    per_depth += SIZES.params * core::mem::size_of::<VmOutBinding>();
    per_depth += SIZES.params * core::mem::size_of::<VmFbOutBinding>();
    per_depth += SIZES.params * core::mem::size_of::<Option<(VmWriteTarget, Option<u32>)>>();
    per_depth += SIZES.params * core::mem::size_of::<(VmWriteTarget, Value)>();
    per_depth += SIZES.args * core::mem::size_of::<Value>();
    let table = KIND_COUNT * 2 * core::mem::size_of::<Vec<u8>>();
    assert_eq!(bytes, table + 2 * per_depth, "every kind is charged");
}

/// Spec 34 MEM-09 (§6.3 call scratch; census C9): the standard-library values come from the
/// reserved scratch, sized for the most actual arguments of any call site, so a take within the
/// reservation makes no allocation and the vector returns on recycle. Catches a kind that
/// allocates on take, is sized below the call it must serve, or is dropped rather than pooled on
/// return.
#[test]
fn standard_values_come_from_the_reserved_scratch() {
    let mut scratch = CallScratch::default();
    scratch.reserve(1, SIZES).unwrap();

    let mut values = scratch.take_stdlib_values();
    assert_eq!(values.capacity(), SIZES.args, "standard-library values");
    let pointer = values.as_ptr();
    for index in 0..SIZES.args {
        values.push(Value::DInt(index as i32));
    }
    assert_eq!(
        values.capacity(),
        SIZES.args,
        "the reservation holds the call"
    );
    assert!(
        core::ptr::eq(values.as_ptr(), pointer),
        "a take within the reservation makes no allocation"
    );

    scratch.recycle_stdlib_values(values);
    assert_eq!(
        scratch.held_counts()[KIND_STDLIB_VALUES],
        1,
        "the vector returns to the pool"
    );
    let reused = scratch.take_stdlib_values();
    assert_eq!(
        reused.capacity(),
        SIZES.args,
        "the reserved storage is reused"
    );
    assert_eq!(reused, Vec::<Value>::new(), "a returned vector is emptied");
    assert_eq!(scratch.most_outstanding()[KIND_STDLIB_VALUES], 1);
}
