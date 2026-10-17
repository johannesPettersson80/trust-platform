use super::*;
use crate::error::ExecutionLimitKind;
use crate::value::Duration;
use crate::vm::PreparationLimits;

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
