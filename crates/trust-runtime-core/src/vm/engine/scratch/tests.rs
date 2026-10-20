use super::*;
use crate::error::ExecutionLimitKind;
use crate::value::Duration;
use crate::vm::call::bindings::{bind_vm_call_arguments, VmNativeArg, VmNativeArgValue};
use crate::vm::call::scratch::{KIND_COUNT, KIND_FLAGS, KIND_FUNCTION_OUTPUTS};
use crate::vm::PreparationLimits;
use crate::vm::VmTrap;

const MAIN: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
);
const NUMERIC: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
);
const GPIO: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc");
const SCALAR: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/scalar/program.stbc");
const CALLS: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/calls/program.stbc");
const BLOCKS: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/blocks/program.stbc");
const METHODS: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/methods/program.stbc");
const FAULTS: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/faults/program.stbc");
const RETAIN: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/retain/program.stbc");
const REFERENCES: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/references/program.stbc");

/// The chips' call depth, with the hosted preparation allowances the register tier needs.
fn prepared(bytes: &[u8]) -> PreparedModule {
    PreparedModule::from_bytes(
        bytes,
        PreparationLimits {
            max_call_depth: 4,
            ..PreparationLimits::default()
        },
    )
    .unwrap()
}

/// Spec 34 MEM-02 and MEM-09: the scratch is reserved at construction and charged against the
/// construction allowance, so an allowance short of it refuses the construction, and the call
/// levels follow from the program's static call depth, not the admitted ceiling (main's
/// function-block and method calls are two frames under the test ceiling of four).
#[test]
fn construction_charges_the_reserved_scratch_against_its_allowance() {
    for (name, bytes, levels) in [("main", MAIN, 2), ("gpio", GPIO, 4), ("scalar", SCALAR, 1)] {
        let prepared = prepared(bytes);
        let plan = ScratchPlan::of(&prepared).unwrap();
        assert_eq!(plan.levels, levels, "{name}");
        assert!(plan.operands >= prepared.max_operand_depth, "{name}");
        let scratch = plan.bytes().unwrap();
        let data = EngineState::construct_data(&prepared, 0, &services::LOGICAL_ONLY).unwrap();
        let total = data.resources.construction_bytes.get();
        assert!(scratch > 0 && total >= scratch, "{name}");
        let buffers = data.resources.buffers.last().unwrap();
        assert!(
            buffers.operand_stack.storage_capacity() >= plan.operands,
            "{name}"
        );
        let mut short = self::prepared(bytes);
        short.limits.max_construction_bytes = total - scratch;
        assert!(
            matches!(
                EngineState::construct_data(&short, 0, &services::LOGICAL_ONLY),
                Err(RuntimeError::ExecutionLimit {
                    kind: ExecutionLimitKind::AllocationBytes,
                    ..
                })
            ),
            "{name}"
        );
        let mut exact = self::prepared(bytes);
        exact.limits.max_construction_bytes = total;
        assert!(
            EngineState::construct_data(&exact, 0, &services::LOGICAL_ONLY).is_ok(),
            "{name}"
        );
    }
}

/// Spec 34 TIME-08 and MEM-09: every scan of an engine with the reserved scratch has the
/// result, the charges, the remaining work, the images and the storage of an engine whose
/// scratch grows on demand, as it did before the reservation.
#[test]
fn reserved_scratch_changes_no_result_or_charge_of_any_scan() {
    for (name, bytes) in [
        ("main", MAIN),
        ("numeric", NUMERIC),
        ("gpio", GPIO),
        ("scalar", SCALAR),
    ] {
        let prepared = prepared(bytes);
        let services = &services::LOGICAL_ONLY;
        let mut reserved = EngineState::construct_data(&prepared, 0, services).unwrap();
        let mut grown = EngineState::construct_data(&prepared, 0, services).unwrap();
        let resources = &mut grown.resources;
        resources.buffers = resources
            .buffers
            .iter()
            .map(ExecutionBuffers::grown_on_demand)
            .collect();
        resources.frame_locals = FrameLocals::default();
        resources.spare_journal = SavedDestinations::new();
        resources.staging = Default::default();
        for scan in 0..101 {
            let now = Duration::from_millis(10 * scan);
            let expected = EngineState::borrow(&mut grown, &prepared, services).execute_cycle(now);
            let actual = EngineState::borrow(&mut reserved, &prepared, services).execute_cycle(now);
            assert_eq!(actual, expected, "{name} scan {scan}");
            assert_eq!(
                reserved.resources.construction_bytes.get(),
                grown.resources.construction_bytes.get(),
                "{name} scan {scan}"
            );
            assert_eq!(
                reserved.resources.work_budget.remaining(),
                grown.resources.work_budget.remaining(),
                "{name} scan {scan}"
            );
            assert_eq!(reserved.images.outputs, grown.images.outputs);
            assert_eq!(reserved.images.memory, grown.images.memory);
            assert_eq!(
                alloc::format!("{:?}", reserved.storage),
                alloc::format!("{:?}", grown.storage),
                "{name} scan {scan}"
            );
        }
    }
}

/// Spec 34 MEM-02 and MEM-09: a program run with reservations sized to its static call depth
/// has the same result, per-scan charges, remaining work, images, storage and refusals as the
/// same program with ceiling-sized reservations, over 101 scans of the same fixtures.
#[test]
fn depth_sized_reservations_change_no_result_or_charge_of_any_scan() {
    for (name, bytes) in [
        ("main", MAIN),
        ("numeric", NUMERIC),
        ("gpio", GPIO),
        ("scalar", SCALAR),
    ] {
        let depth_sized = prepared(bytes);
        // A recursive call graph reserves for the ceiling; forcing that variant gives the
        // ceiling-sized arm without changing the program's behaviour.
        let mut ceiling_sized = prepared(bytes);
        ceiling_sized.call_depth = crate::bytecode::CallDepth::Recursive;
        let services = &services::LOGICAL_ONLY;
        let mut depth_data = EngineState::construct_data(&depth_sized, 0, services).unwrap();
        let mut ceiling_data = EngineState::construct_data(&ceiling_sized, 0, services).unwrap();
        for scan in 0..101 {
            let now = Duration::from_millis(10 * scan);
            let expected =
                EngineState::borrow(&mut ceiling_data, &ceiling_sized, services).execute_cycle(now);
            let actual =
                EngineState::borrow(&mut depth_data, &depth_sized, services).execute_cycle(now);
            assert_eq!(actual, expected, "{name} scan {scan}");
            assert_eq!(
                depth_data.resources.construction_bytes.get(),
                ceiling_data.resources.construction_bytes.get(),
                "{name} scan {scan}"
            );
            assert_eq!(
                depth_data.resources.work_budget.remaining(),
                ceiling_data.resources.work_budget.remaining(),
                "{name} scan {scan}"
            );
            assert_eq!(depth_data.images.outputs, ceiling_data.images.outputs);
            assert_eq!(depth_data.images.memory, ceiling_data.images.memory);
            assert_eq!(
                alloc::format!("{:?}", depth_data.storage),
                alloc::format!("{:?}", ceiling_data.storage),
                "{name} scan {scan}"
            );
        }
    }
}

/// Spec 34 MEM-09 (census A3): the program activation takes the local slots reserved at
/// construction and returns them, so no scan allocates them again.
#[test]
fn the_program_activation_reuses_its_reserved_locals() {
    let prepared = prepared(SCALAR);
    let services = &services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let reserved = data.resources.frame_locals.spares.last().unwrap().as_ptr();
    let room = data.resources.frame_locals.spares.capacity();
    for scan in 0..5 {
        EngineState::borrow(&mut data, &prepared, services)
            .execute_cycle(Duration::from_millis(10 * scan))
            .unwrap();
        let spares = &data.resources.frame_locals.spares;
        assert!(
            spares.iter().any(|locals| locals.as_ptr() == reserved),
            "scan {scan}"
        );
        assert_eq!(spares.capacity(), room, "scan {scan}");
    }
}

/// Replace every reserved scratch with empty storage, as a run without the reservation
/// holds, keeping the construction charge. This is the fresh-vector arm of the call-path
/// differential: an engine that allocates its frame payloads, frame locals, edge lists and
/// call locals on demand.
fn grown_on_demand(data: &mut EngineData) {
    let resources = &mut data.resources;
    resources.buffers = resources
        .buffers
        .iter()
        .map(ExecutionBuffers::grown_on_demand)
        .collect();
    resources.frame_locals = FrameLocals::default();
    resources.edge_inputs = alloc::vec::Vec::new();
    resources.edge_restores = alloc::vec::Vec::new();
    resources.call_scratch = crate::vm::call::scratch::CallScratch::default();
    resources.spare_journal = SavedDestinations::new();
    resources.staging = Default::default();
}

/// Spec 34 MEM-03, MEM-09 and TIME-08 (census A3, A4, A5 and C5): the call fixtures run
/// with the per-depth frame payloads, the bound call locals and the reserved edge lists
/// exactly as they run with fresh vectors, field by field over 101 scans. Catches a changed
/// result, charge, remaining work, image or refusal, and a payload reused without being
/// cleared between activations.
#[test]
fn frame_payload_reuse_changes_no_result_or_charge_of_any_scan() {
    for (name, bytes) in [("calls", CALLS), ("blocks", BLOCKS), ("methods", METHODS)] {
        let prepared = prepared(bytes);
        let services = &services::LOGICAL_ONLY;
        let mut reserved = EngineState::construct_data(&prepared, 0, services).unwrap();
        let mut fresh = EngineState::construct_data(&prepared, 0, services).unwrap();
        grown_on_demand(&mut fresh);
        for scan in 0..101 {
            let now = Duration::from_millis(10 * scan);
            let expected = EngineState::borrow(&mut fresh, &prepared, services).execute_cycle(now);
            let actual = EngineState::borrow(&mut reserved, &prepared, services).execute_cycle(now);
            assert_eq!(actual, expected, "{name} scan {scan}");
            assert_eq!(
                reserved.resources.construction_bytes.get(),
                fresh.resources.construction_bytes.get(),
                "{name} scan {scan}"
            );
            assert_eq!(
                reserved.resources.work_budget.remaining(),
                fresh.resources.work_budget.remaining(),
                "{name} scan {scan}"
            );
            assert_eq!(reserved.images.outputs, fresh.images.outputs);
            assert_eq!(reserved.images.memory, fresh.images.memory);
            assert_eq!(
                alloc::format!("{:?}", reserved.storage),
                alloc::format!("{:?}", fresh.storage),
                "{name} scan {scan}"
            );
        }
    }
}

/// Spec 34 MEM-02 and MEM-09 (census A3, A4, A5 and C5): a call chain one level past the
/// admitted depth is refused at `max_call_depth` with the same error whether the frames
/// take reserved payloads or fresh vectors. The `faults` fixture's `fault_kind` 6 runs its
/// four-deep chain one level past the chips' admitted depth; the corpus has no recursive
/// program, and the refusal is the one a cyclic call graph reaches. Catches a reservation
/// that moves the refusal depth or its error.
#[test]
fn recursion_beyond_the_static_depth_is_refused_as_today() {
    let prepared = prepared(FAULTS);
    assert_eq!(
        prepared.limits.max_call_depth, 4,
        "the chips' admitted depth"
    );
    for fresh in [false, true] {
        let services = &services::LOGICAL_ONLY;
        let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
        if fresh {
            grown_on_demand(&mut data);
        }
        EngineState::borrow(&mut data, &prepared, services)
            .execute_cycle(Duration::ZERO)
            .expect("the healthy first scan");
        EngineState::borrow(&mut data, &prepared, services)
            .write_global("fault_kind", crate::value::Value::Int(6))
            .expect("the fault_kind write");
        let error = EngineState::borrow(&mut data, &prepared, services)
            .execute_cycle(Duration::from_millis(10))
            .expect_err("the depth refusal");
        assert_eq!(
            error.stable_code().as_str(),
            "vm_call_stack_overflow",
            "fresh={fresh}"
        );
    }
}

/// Spec 34 MEM-09, MEM-03 and TIME-08 (census C1, C3, C4, C5 and C7): the call fixtures run
/// with the per-depth reserved call scratch exactly as they run with fresh binding vectors,
/// field by field over 101 scans. Catches a changed result, charge, remaining work, image or
/// refusal, and a scratch reused without being cleared between calls.
#[test]
fn call_scratch_reuse_changes_no_result_or_charge_of_any_scan() {
    for (name, bytes) in [("calls", CALLS), ("blocks", BLOCKS), ("methods", METHODS)] {
        let prepared = prepared(bytes);
        let services = &services::LOGICAL_ONLY;
        let mut reserved = EngineState::construct_data(&prepared, 0, services).unwrap();
        let mut fresh = EngineState::construct_data(&prepared, 0, services).unwrap();
        grown_on_demand(&mut fresh);
        for scan in 0..101 {
            let now = Duration::from_millis(10 * scan);
            let expected = EngineState::borrow(&mut fresh, &prepared, services).execute_cycle(now);
            let actual = EngineState::borrow(&mut reserved, &prepared, services).execute_cycle(now);
            assert_eq!(actual, expected, "{name} scan {scan}");
            assert_eq!(
                reserved.resources.construction_bytes.get(),
                fresh.resources.construction_bytes.get(),
                "{name} scan {scan}"
            );
            assert_eq!(
                reserved.resources.work_budget.remaining(),
                fresh.resources.work_budget.remaining(),
                "{name} scan {scan}"
            );
            assert_eq!(reserved.images.outputs, fresh.images.outputs);
            assert_eq!(reserved.images.memory, fresh.images.memory);
            assert_eq!(
                alloc::format!("{:?}", reserved.storage),
                alloc::format!("{:?}", fresh.storage),
                "{name} scan {scan}"
            );
        }
    }
}

/// Spec 34 MEM-09 (§6.3 call scratch): a call refused by a type check returns its reserved
/// scratch, and the next call takes the same reserved vectors. The `calls` fixture's `Digits`
/// declares two outputs; supplying them as expressions instead of references makes the output
/// preflight refuse the call with a type mismatch. Catches a refusal path that drops a taken
/// scratch.
#[test]
fn a_refused_call_returns_its_scratch() {
    let prepared = prepared(CALLS);
    let services = &services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let reserved = data.resources.call_scratch.held_counts();
    let digits = prepared
        .vm
        .pou_by_id
        .iter()
        .find_map(|(id, _)| {
            prepared
                .vm
                .pou_name(*id)
                .is_some_and(|name| name.eq_ignore_ascii_case("Digits"))
                .then_some(*id)
        })
        .expect("the calls fixture declares the Digits function");

    // `Digits(value, low, high)`: the two outputs are supplied as expressions, which the output
    // preflight refuses as a type mismatch before any caller mutation.
    let args = [
        VmNativeArg {
            name: None,
            value: VmNativeArgValue::Expr(Value::DInt(5)),
        },
        VmNativeArg {
            name: None,
            value: VmNativeArgValue::Expr(Value::DInt(0)),
        },
        VmNativeArg {
            name: None,
            value: VmNativeArgValue::Expr(Value::DInt(0)),
        },
    ];
    let frame = VmFrame {
        activation: None,
        pou_id: Some(0),
        return_pc: 0,
        code_start: 0,
        code_end: 0,
        local_ref_start: 0,
        local_ref_count: 0,
        locals: Vec::new(),
        parameter_values_present: Vec::new(),
        runtime_instance: None,
        instance_owner: None,
    };
    let error = bind_vm_call_arguments(
        &mut EngineState::borrow(&mut data, &prepared, services),
        &prepared.vm,
        &frame,
        digits,
        &args,
    )
    .unwrap_err();
    assert!(
        matches!(error, VmTrap::Runtime(RuntimeError::TypeMismatch)),
        "the output preflight refuses with a type mismatch, got {error:?}"
    );

    assert_eq!(
        data.resources.call_scratch.held_counts(),
        reserved,
        "a refused call returns every scratch vector it took"
    );
}

/// Spec 34 MEM-09 (§6.3 call scratch): nested calls take their own depth's scratch, so the
/// `calls` fixture's `Twice` holds its scratch while the `Scale` it calls takes another. Catches
/// a scratch that a nested call shares with its caller, which would corrupt one of them.
#[test]
fn nested_calls_use_their_own_depths_scratch() {
    let prepared = prepared(CALLS);
    let services = &services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    EngineState::borrow(&mut data, &prepared, services)
        .execute_cycle(Duration::from_millis(10))
        .expect("the nested-calls scan");

    let most = data.resources.call_scratch.most_outstanding();
    assert!(
        most[KIND_FUNCTION_OUTPUTS] >= 2,
        "Twice holds a scratch while Scale takes another, got {:?}",
        most
    );
    assert!(
        most[KIND_FLAGS] >= 2,
        "the presence bits of a caller and a callee are held at once, got {most:?}"
    );
    assert_eq!(
        data.resources.call_scratch.held_counts(),
        [prepared.reserved_call_levels(); KIND_COUNT],
        "every nested call returns its scratch"
    );
}

/// Spec 34 MEM-09 (C1-19c): the retain capture is reserved for the largest payload of the
/// resource. For the `retain` corpus program, worked out by hand from `retain/src/main.st`: the
/// entry count, 4 bytes; the globals `counter` and `setpoint`, each its name with a 4-byte length
/// and a scalar's tag with at most 8 bytes, 4 + 7 + 9 = 20 and 4 + 8 + 9 = 21; the program `Plant`'s
/// `runs` under `@program/Plant/runs`, 4 + 19 + 9 = 32; and its `label`, a `STRING[12]`, under
/// `@program/Plant/label` with a tag, a 4-byte length and 12 bytes, 4 + 20 + 17 = 41. That is 118
/// bytes. Catches a bound that leaves the program variables out (45), one that counts the text
/// by its current value instead of its capacity (112) and one without the names (36 fewer).
#[test]
fn the_retain_capture_is_reserved_for_the_largest_payload() {
    let prepared = prepared(RETAIN);
    let services = &services::LOGICAL_ONLY;
    let mut data = EngineState::construct_data(&prepared, 0, services).unwrap();
    let state = EngineState::borrow(&mut data, &prepared, services);
    assert_eq!(state.retained_payload_bound(), Some(118));
}

/// Spec 34 MEM-09 (C1-19c3): construction keeps a path buffer for every reference a scan can hold
/// at once. For the `references` corpus program, worked out by hand from
/// `references/src/main.st`: its statements hold at most two references at once (`whole^ :=
/// whole^ + 1` and `member^ := member^ + element^` keep the destination's reference while they load
/// through another), it makes no call and has no frame variable, and one more buffer takes a path
/// that moves into a larger one: 3. The program's five `REF_TO DINT` variables lie in the
/// persistent block, each a leaf whose reference may keep a path: 5 more, 8 in all. A path goes
/// through at most one member (`reading.amount`) or one array level (`store[slot]`), so each path
/// buffer has room for one segment and each of the 8 index buffers for one subscript. Catches a
/// pool sized for the operands alone (3) or the persistent leaves alone (5), one not reserved (0,
/// so the first scan allocates) and buffers without room for the longest path.
#[test]
fn the_reference_pool_holds_every_reference_a_scan_can_hold() {
    let prepared = prepared(REFERENCES);
    let plan = ScratchPlan::of(&prepared).unwrap();
    assert_eq!(plan.reference_paths, 3);
    assert_eq!(
        plan.path_shape,
        crate::vm::prepared::operand_bounds::PathShape {
            segments: 1,
            indexes: 1,
            subscripts: 1,
        }
    );
    let data = EngineState::construct_data_with(
        &prepared,
        0,
        &services::LOGICAL_ONLY,
        Construction {
            profile: crate::value::DateTimeProfile::default(),
        },
    )
    .unwrap();
    assert_eq!(data.resources.references.kept(), (8, 8));
}

/// Spec 34 MEM-09: a reserved visited set is reused, and it comes back empty. Catches a set handed
/// to an initializer with the instances of the one that used it before: the write policy would
/// let the new initializer write them, and its staging would skip their backups.
#[test]
fn a_returned_visited_set_is_empty_when_taken_again() {
    let mut scratch = BookkeepingScratch::default();
    scratch.reserve(1, 1).unwrap();
    let mut visited = scratch.take_visited();
    assert!(visited.insert(InstanceId(7)).unwrap());
    scratch.return_visited(visited);
    let visited = scratch.take_visited();
    assert!(!visited.contains(&InstanceId(7)));
    assert_eq!(visited.iter().count(), 0);
    scratch.return_visited(visited);
    assert!(scratch.all_buffers_returned());
}
