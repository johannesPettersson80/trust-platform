//! Spec 34 MEM-03, MEM-09 and TIME-08 (bounded-c slice C1-4, census N1-N4, I10 and I11):
//! initializer evaluation, construction frames, the lifetime check and `SIZEOF` reuse scratch
//! reserved at construction, and every result, charge, remaining work, image and refusal is
//! unchanged.
use super::*;
use crate::value::Duration;
use crate::vm::PreparationLimits;

const INITIALIZERS: &[u8] = include_bytes!(
    "../../../../../trust-runtime/tests/fixtures/bounded-c/initializers/program.stbc"
);
const SIZEOF: &[u8] =
    include_bytes!("../../../../../trust-runtime/tests/fixtures/bounded-c/sizeof/program.stbc");

/// Spec 34 MEM-09 and TIME-08: over 101 scans of the bounded-C fixtures with initializers and
/// `SIZEOF`, an engine whose bookkeeping scratch is reserved produces the same result, per-scan
/// charges, remaining work, images and storage as one whose scratch is fresh each evaluation and
/// grows on demand, as it did before C1-4. Catches a reservation that skips a charge, changes a
/// result or reuses a buffer without clearing it.
#[test]
fn bookkeeping_scratch_changes_no_result_or_charge_of_any_scan() {
    for (name, bytes) in [("initializers", INITIALIZERS), ("sizeof", SIZEOF)] {
        let prepared = PreparedModule::from_bytes(bytes, PreparationLimits::default()).unwrap();
        let services = &super::super::services::LOGICAL_ONLY;
        let mut reserved = EngineState::construct_data(&prepared, 0, services).unwrap();
        let mut grown = EngineState::construct_data(&prepared, 0, services).unwrap();
        // The fresh arm hands out empty buffers that grow on demand, as before C1-4.
        grown.resources.bookkeeping = super::super::scratch::BookkeepingScratch::default();
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
            assert_eq!(
                reserved.images.outputs, grown.images.outputs,
                "{name} scan {scan}"
            );
            assert_eq!(
                reserved.images.memory, grown.images.memory,
                "{name} scan {scan}"
            );
            assert_eq!(
                alloc::format!("{:?}", reserved.storage),
                alloc::format!("{:?}", grown.storage),
                "{name} scan {scan}"
            );
        }
    }
}

/// Spec 34 MEM-09 and TIME-08: a trap inside an initializer body returns the staged result slot,
/// the seed's pending, visited and backup lists and the snapshot buffer to the reserved scratch.
/// Catches a trap path that releases the frame without returning the buffers it took.
#[test]
fn a_trap_inside_an_initializer_returns_its_scratch() {
    let mut prepared = PreparedModule::from_bytes(
        include_bytes!(
            "../../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
        crate::vm::PreparationLimits::default(),
    )
    .unwrap();
    let mut data =
        EngineState::construct_data(&prepared, 0, &super::super::services::LOGICAL_ONLY).unwrap();
    let program = crate::vm::engine::test_store::global_instance(
        &EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY),
        "Plant",
    );
    let pou = *data.construction.instance_templates.get(&program).unwrap();
    // Stage the member that owns a nested instance, the program's `Counter`, so the seed takes
    // its pending, visited and backup lists and backs that instance up.
    let declaration = prepared
        .storage_declarations()
        .position(|entry| {
            entry.owner == StorageOwner::Instance
                && entry.owner_pou_id == Some(pou)
                && prepared.vm.strings[entry.name_idx as usize].eq_ignore_ascii_case("counter")
        })
        .unwrap();
    let code_offset = prepared.vm.code.len() as u32;
    prepared.vm.code.push(0xFF); // malformed instruction
    let mut edited = prepared.initializer_at(0);
    edited.code_offset = code_offset;
    edited.code_length = 1;
    edited.stage = crate::bytecode::InitializationStage::Explicit;
    edited.declaration_idx = Some(declaration as u32);
    prepared.set_initializer(0, edited);
    let mut state =
        EngineState::borrow(&mut data, &prepared, &super::super::services::LOGICAL_ONLY);
    assert_eq!(
        state
            .evaluate_initializer(0, None, Some(program), 0)
            .unwrap_err()
            .stable_code(),
        crate::error::StableErrorCode::BytecodeInvalidOpcode
    );
    // Every reserved list is back at its construction length.
    assert!(state.data.resources.bookkeeping.all_buffers_returned());
    assert!(state.data.construction.initializers.is_empty());
    assert_eq!(state.data.storage.execution_frame_count(), 0);
}

/// Spec 34 TIME-08: `SIZEOF(type)` charges 1 unit plus its nesting depth per type node, plus 1
/// per array dimension, exactly as before C1-4. The `sizeof` fixture's `Block` is DINT,
/// `ARRAY[0..3] OF INT` and `STRING[12]`: 1 + 2 + 2 + 1 + 3 + 2 = 11 units and 24 bytes.
/// Catches a dropped dimension, a skipped alias/node or a size computed off the wrong field.
#[test]
fn sizeof_charges_the_same_work_as_before() {
    let prepared = PreparedModule::from_bytes(SIZEOF, PreparationLimits::default()).unwrap();
    let type_idx = prepared
        .named_type("Block")
        .expect("the fixture declares Block");
    let mut units = 0usize;
    let size = crate::vm::dispatch_sizeof::sizeof_type_from_table_with(
        &prepared.vm.types,
        type_idx,
        &mut |charge| -> Result<(), RuntimeError> {
            units += charge;
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(size, 24, "DINT 4 + ARRAY[0..3] OF INT 8 + STRING[12] 12");
    assert_eq!(
        units, 11,
        "struct 1 + DINT 2 + array 2 + dimension 1 + INT 3 + STRING 2"
    );
}
