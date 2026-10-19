//! Tests of the cell store's construction in the engine (specification 34 §6.2, MEM-10; C1-9d2b-1).
//!
//! `EngineState::build` constructs the cell arm when it is asked to: the arm is charged, reserved once and
//! filled by the same construction steps as the `Value` store. These tests build programs on both stores
//! and read the cells through the store-access seam. The bounded-C corpus supplies compiled programs; the
//! `Keeper` module (`keeper.rs`) supplies references in aggregates and dates, which no corpus program has.
//! Each expected value is worked out by hand from the ST source it names. The scans of the whole corpus on
//! both stores are in `scans.rs`, and the engineering and host paths that a scan does not reach are in
//! `engineering.rs` (C1-9d3a).

mod call_instances;
mod engineering;
mod flat_references;
mod frames;
mod identities;
mod keeper;
mod retain;
mod scans;

use super::{services, Construction, EngineData, EngineState, EngineStore, StoreKind};
use crate::bytecode::{StorageOwner, StorageRole, ValidationLimits};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::cell_access::{fixture, CellArm};
use crate::memory::layout::{ByteRange, LayoutRegion};
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{Generation, InstanceId, MemoryLocation};
use crate::retain::RestartMode;
use crate::value::{
    ArrayValue, DateTimeProfile, DateTimeValue, DateValue, Duration, RefSegment, StructValue,
    Value, ValueRef, ValueRefView,
};
use crate::vm::{PreparationLimits, PreparedModule};
use smol_str::SmolStr;
use std::sync::Arc;

const SCALAR: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/scalar/program.stbc");
const INITIALIZERS: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/initializers/program.stbc");
const BLOCKS: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/blocks/program.stbc");
const STRINGS: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/strings/program.stbc");
const RETAIN: &[u8] =
    include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/retain/program.stbc");

/// Every program of the bounded-C corpus and the three saved artifacts the boards run, by name.
const CORPUS: [(&str, &[u8]); 23] = [
    ("scalar", SCALAR),
    (
        "gpio",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/gpio/program.stbc"),
    ),
    (
        "io",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/io/program.stbc"),
    ),
    (
        "calls",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/calls/program.stbc"),
    ),
    ("blocks", BLOCKS),
    (
        "methods",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/methods/program.stbc"),
    ),
    (
        "instances",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/instances/program.stbc"),
    ),
    (
        "arrays",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/arrays/program.stbc"),
    ),
    (
        "structures",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/structures/program.stbc"
        ),
    ),
    (
        "references",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/references/program.stbc"
        ),
    ),
    ("strings", STRINGS),
    ("initializers", INITIALIZERS),
    (
        "sizeof",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/sizeof/program.stbc"),
    ),
    ("retain", RETAIN),
    (
        "retained_blocks",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/retained_blocks/program.stbc"
        ),
    ),
    (
        "retained_values",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/bounded-c/retained_values/program.stbc"
        ),
    ),
    (
        "faults",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/faults/program.stbc"),
    ),
    (
        "copies",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/copies/program.stbc"),
    ),
    (
        "overlays",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/overlays/program.stbc"),
    ),
    (
        "unions",
        include_bytes!("../../../../trust-runtime/tests/fixtures/bounded-c/unions/program.stbc"),
    ),
    (
        "saved/main",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/program-v2.stbc"
        ),
    ),
    (
        "saved/numeric",
        include_bytes!(
            "../../../../trust-runtime/tests/fixtures/portability/stbc-2.0/numeric-v2.stbc"
        ),
    ),
    (
        "saved/gpio",
        include_bytes!("../../../../trust-runtime/tests/fixtures/portability/f401/gpio.stbc"),
    ),
];

/// The bytes of the corpus program `name`.
fn corpus(name: &str) -> &'static [u8] {
    CORPUS
        .iter()
        .find(|(entry, _)| *entry == name)
        .map(|(_, bytes)| *bytes)
        .unwrap_or_else(|| panic!("the corpus holds {name}"))
}

fn prepare(bytes: &[u8]) -> PreparedModule {
    PreparedModule::from_bytes(bytes, PreparationLimits::default()).expect("the program prepares")
}

/// The admission limits of the chips, as `tests/scan_allocation_zero.rs` states them.
fn chip_limits() -> PreparationLimits {
    PreparationLimits {
        max_artifact_bytes: 16 * 1024,
        max_preparation_bytes: 512 * 1024,
        max_preparation_work: 1_000_000,
        validation: ValidationLimits {
            max_scratch_bytes: 48 * 1024,
            max_work: 500_000,
        },
        max_construction_values: 4096,
        max_construction_bytes: 128 * 1024,
        max_process_image_bytes: 64,
        max_work: 50_000,
        max_construction_work: 50_000,
        max_call_depth: 4,
    }
}

/// The engine data of `prepared` built on `store`, with the default profile.
fn build(prepared: &PreparedModule, store: StoreKind) -> Result<Box<EngineData>, RuntimeError> {
    build_with(prepared, store, DateTimeProfile::default())
}

fn build_with(
    prepared: &PreparedModule,
    store: StoreKind,
    profile: DateTimeProfile,
) -> Result<Box<EngineData>, RuntimeError> {
    EngineState::construct_data_with(
        prepared,
        0,
        &services::LOGICAL_ONLY,
        Construction { store, profile },
    )
}

/// The cell arm a state built on `StoreKind::Cells` holds.
fn arm(data: &EngineData) -> &CellArm {
    match &data.store {
        EngineStore::Cells(arm) => arm,
        EngineStore::Values => panic!("the state holds the Value store"),
    }
}

/// The value of the global `name`.
fn global(state: &EngineState<'_>, name: &str) -> Value {
    StoreAccess::read_global(state, name)
        .unwrap_or_else(|| panic!("the global {name} is declared"))
        .materialize()
        .expect("the global materializes")
}

/// The value of the member `name` of `instance`.
fn member(state: &EngineState<'_>, instance: InstanceId, name: &str) -> Value {
    StoreAccess::read_instance_var(state, instance, name)
        .unwrap_or_else(|| panic!("the instance holds {name}"))
        .materialize()
        .expect("the member materializes")
}

fn declaration_name(prepared: &PreparedModule, declaration: u32) -> &str {
    let entry = &prepared.layout.entries[declaration as usize];
    &prepared.vm.strings[entry.name_idx as usize]
}

/// Every declared variable of `prepared` as `state` holds it: the globals in declaration order, then each
/// persistent instance with its type, its parent and its members. One line per fact, so two stores that
/// agree give equal lists.
fn declared_values(prepared: &PreparedModule, state: &EngineState<'_>) -> Vec<String> {
    // A reference variable is shown by the variable and path that hold its target, as the value
    // store names it: on the flat store it holds a flat reference (C1-11d).
    let named = |value: Result<Value, RuntimeError>| match value {
        Ok(Value::Reference(Some(reference))) => {
            Ok(Value::Reference(Some(state.path_form(&reference))))
        }
        other => other,
    };
    let mut lines = Vec::new();
    for &declaration in prepared.declarations(StorageOwner::Global, None) {
        let name = declaration_name(prepared, declaration);
        let value = StoreAccess::read_global(state, name).map(|view| named(view.materialize()));
        lines.push(format!("global {name} = {value:?}"));
    }
    for (index, root) in prepared.roots.entries.iter().enumerate() {
        let (Some(pou), Some(instance)) =
            (root.template_pou_id, state.data.construction.roots[index])
        else {
            continue;
        };
        lines.push(format!(
            "instance {} of {:?}, parent {:?}",
            instance.0,
            StoreAccess::instance_type_name(state, instance),
            StoreAccess::instance_parent(state, instance),
        ));
        for &declaration in prepared.declarations(StorageOwner::Instance, Some(pou)) {
            let name = declaration_name(prepared, declaration);
            let value = StoreAccess::read_instance_var(state, instance, name)
                .map(|view| named(view.materialize()));
            lines.push(format!("instance {} {name} = {value:?}", instance.0));
        }
    }
    lines
}

#[test]
fn persistent_instances_are_registered_at_construction() {
    // Specification 34 §6.2 (MEM-10; O8): each persistent root has an instance, recorded in the cell arm with
    // an identity from the store's one counter, in CONSTRUCTION_ROOTS order. `initializers` declares
    // `PROGRAM Initializers` (VAR CONSTANT gain : INT; VAR tracker : Tracker; slot, result : INT END_VAR) and
    // `FUNCTION_BLOCK Tracker` (VAR history : ARRAY[1..3] OF INT END_VAR). Its roots are the globals `ticks`,
    // `table`, `origin`, `corners` and `capacity`, the program (template pou 0), the tracker (template pou 34,
    // nested in the program) and the static `calls`. Only the program and the tracker have instances, so they
    // take identities 0 and 1 on the `Value` store, and the cells must give the same. The globals take
    // 26 bytes at their declared sizes (MEM-12: `ticks` a DINT 4, `table` three INTs 6, `origin` two
    // INTs 4, `corners` two of them 8, `capacity` and `calls` an INT 2 each), so the roots start at the
    // next multiple of 8, byte 32. The program holds `gain` (an INT) at 0, `tracker` (an 8-byte
    // instance leaf) at 8, `slot` at 16 and `result` at 18: 20 bytes rounded up to 24, at 32 to 55. The
    // tracker holds `history` (three INTs, 6 bytes rounded up to 8) and follows at byte 56.
    //
    // A second counter for per-call instances would start at 0 and issue identity 0 again: the instance
    // created next must take 2 on both stores. A store that did not record the instances would answer no
    // type name for identity 0 or 1.
    let prepared = prepare(INITIALIZERS);
    let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
    let mut cells = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let roots = vec![
        None,
        None,
        None,
        None,
        None,
        Some(InstanceId(0)),
        Some(InstanceId(1)),
        None,
    ];
    assert_eq!(values.construction.roots, roots);
    assert_eq!(cells.construction.roots, roots);
    assert_eq!(
        arm(&cells).instance_range(InstanceId(0)),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: 32,
            bytes: 24,
        })
    );
    assert_eq!(
        arm(&cells).instance_range(InstanceId(1)),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: 56,
            bytes: 8,
        })
    );
    assert_eq!(arm(&cells).instance_range(InstanceId(2)), None);

    let values = EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY);
    let mut cells = EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY);
    for instance in [InstanceId(0), InstanceId(1)] {
        let name = StoreAccess::instance_type_name(&cells, instance).cloned();
        assert!(name.is_some(), "instance {instance:?} is recorded");
        assert_eq!(
            name.as_ref(),
            StoreAccess::instance_type_name(&values, instance)
        );
        assert!(matches!(
            StoreAccess::instance_parent(&cells, instance),
            Ok(None)
        ));
    }
    assert_eq!(global(&cells, "Plant"), Value::Instance(InstanceId(0)));
    assert_eq!(
        member(&cells, InstanceId(0), "tracker"),
        Value::Instance(InstanceId(1))
    );
    assert_eq!(
        StoreMutate::try_create_instance(&mut cells, "Scratch"),
        Ok(InstanceId(2))
    );
}

#[test]
fn cells_use_the_engine_date_time_profile() {
    // Specification 34 §6.2 and IEC 61131-3 Ed.3 Table 10 (the initial value of DATE and DT): a DATE and a
    // DATE_AND_TIME default to the epoch of the engine's profile. `Keeper` declares `day_g : DATE` and
    // `stamp_g : DATE_AND_TIME`. For a profile whose epoch is 1,000 ticks both cells hold 1,000 once the
    // store is installed, before any initializer runs; the default profile gives 0. A store built with
    // `DateTimeProfile::default()` whatever the engine runs with would read 0 in both cases.
    //
    // The built state agrees: the engine applies its profile before it constructs anything, so the
    // initializers that follow default the same leaves to the same epoch, on both stores.
    let prepared = keeper::prepared();
    let epoch = DateTimeProfile {
        epoch: DateValue::new(1_000),
        resolution: Duration::from_millis(1),
    };
    for (profile, ticks) in [(epoch, 1_000), (DateTimeProfile::default(), 0)] {
        let resource = prepared.resources.resources[0].clone();
        let mut data = EngineState::new_data(
            &prepared,
            &services::LOGICAL_ONLY,
            resource,
            0,
            Generation::FIRST,
            false,
            Duration::ZERO,
        );
        data.profile = profile;
        let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        state.install_cell_store().expect("the cell store installs");
        assert_eq!(global(&state, "day_g"), Value::Date(DateValue::new(ticks)));
        assert_eq!(
            global(&state, "stamp_g"),
            Value::Dt(DateTimeValue::new(ticks))
        );
    }
    for store in [StoreKind::Values, StoreKind::Cells] {
        let mut data = build_with(&prepared, store, epoch).expect("the program constructs");
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        assert_eq!(global(&state, "day_g"), Value::Date(DateValue::new(1_000)));
        assert_eq!(
            global(&state, "stamp_g"),
            Value::Dt(DateTimeValue::new(1_000))
        );
    }
}

fn array(elements: Vec<Value>, dims: Vec<(i64, i64)>) -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(elements, dims)))
}

fn ints(values: &[i16]) -> Value {
    let high = i64::try_from(values.len()).expect("a short list");
    array(
        values.iter().copied().map(Value::Int).collect(),
        vec![(1, high)],
    )
}

/// The `Pair` of the `initializers` program, a structure of the INT members `a` and `b`.
fn pair(a: i16, b: i16) -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("a"), Value::Int(a));
    fields.insert(SmolStr::new("b"), Value::Int(b));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Pair"),
        fields,
    )))
}

#[test]
fn explicit_initializers_land_in_the_cells() {
    // Specification 34 §6.2 (MEM-10) and IEC 61131-3 Ed.3 §6.5.2.2 (initial values): the value a declaration
    // names lands in its cells, whichever kind of variable it is. The expected values are the ones the ST
    // sources of the corpus declare.
    //
    // initializers: `ticks : DINT := DINT#0`, `table : ARRAY[1..3] OF INT := [4, 5, 6]`; the program's
    // `slot : INT := INT#1` (`result : INT` has none and defaults to 0) and the function block's
    // `history : ARRAY[1..3] OF INT := [1, 2, 3]`. The program is instance 0 and the tracker instance 1.
    // A cell store that took the default over the initializer would read 0 and [0, 0, 0].
    //
    // The structure and constant initializers of C1-9d2b-2 (R6), on the structure `Pair` of the INT
    // members `a` and `b`: the global `origin : Pair := (a := INT#1, b := INT#2)` reads the pair (1, 2);
    // the global `corners : ARRAY[1..2] OF Pair := [(a := INT#3, b := INT#4), (a := INT#5, b := INT#6)]`
    // reads the pairs (3, 4) and (5, 6) in element order; the constant in a global block, `capacity : INT
    // := INT#9`, reads 9; and the constant in the program, `gain : INT := INT#2`, reads 2. A store that
    // wrote a structure's members in the wrong order would read (2, 1), one that wrote the elements in
    // the wrong order would read (5, 6) before (3, 4), and one that left a constant at its default would
    // read 0. The `Value` store is read with the same expectations, since the specification names one
    // value for both.
    let prepared = prepare(INITIALIZERS);
    for kind in [StoreKind::Values, StoreKind::Cells] {
        let mut data = build(&prepared, kind).expect("the store constructs");
        let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
        assert_eq!(global(&state, "ticks"), Value::DInt(0), "{kind:?}");
        assert_eq!(global(&state, "table"), ints(&[4, 5, 6]), "{kind:?}");
        assert_eq!(global(&state, "origin"), pair(1, 2), "{kind:?}");
        assert_eq!(
            global(&state, "corners"),
            array(vec![pair(3, 4), pair(5, 6)], vec![(1, 2)]),
            "{kind:?}"
        );
        assert_eq!(global(&state, "capacity"), Value::Int(9), "{kind:?}");
        assert_eq!(
            member(&state, InstanceId(0), "slot"),
            Value::Int(1),
            "{kind:?}"
        );
        assert_eq!(
            member(&state, InstanceId(0), "result"),
            Value::Int(0),
            "{kind:?}"
        );
        assert_eq!(
            member(&state, InstanceId(0), "gain"),
            Value::Int(2),
            "{kind:?}"
        );
        assert_eq!(
            member(&state, InstanceId(1), "history"),
            ints(&[1, 2, 3]),
            "{kind:?}"
        );
    }

    // scalar: `gain : REAL := REAL#1.5`, `mask : WORD := WORD#16#00FF`, `elapsed : TIME := T#0ms` and
    // `level : LREAL := LREAL#0.0`, each global of its own kind. The program is instance 0 and its
    // `phase : INT := INT#0` and `ready : BOOL := FALSE` start at 0 and false.
    let prepared = prepare(SCALAR);
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    assert_eq!(global(&state, "gain"), Value::Real(1.5));
    assert_eq!(global(&state, "mask"), Value::Word(0x00FF));
    assert_eq!(global(&state, "elapsed"), Value::Time(Duration::ZERO));
    assert_eq!(global(&state, "level"), Value::LReal(0.0));
    assert_eq!(member(&state, InstanceId(0), "phase"), Value::Int(0));
    assert_eq!(member(&state, InstanceId(0), "ready"), Value::Bool(false));

    // strings: the program's `name : STRING[16] := 'pump'`, `wide : WSTRING[16] := "valve"`,
    // `number : DINT := DINT#42` and `reading : REAL := REAL#1.25`; `line : STRING := ''` is empty.
    let prepared = prepare(STRINGS);
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let program = InstanceId(0);
    assert_eq!(
        member(&state, program, "name"),
        Value::String(SmolStr::new("pump"))
    );
    assert_eq!(
        member(&state, program, "wide"),
        Value::WString(String::from("valve"))
    );
    assert_eq!(member(&state, program, "number"), Value::DInt(42));
    assert_eq!(member(&state, program, "reading"), Value::Real(1.25));
    assert_eq!(
        member(&state, program, "line"),
        Value::String(SmolStr::new(""))
    );

    // blocks: the function block `Pulse` (instance 1) declares `increment : INT := INT#1`, and the program
    // `toggle : BOOL := FALSE`, `slot : INT := INT#1` and `counts : ARRAY[0..1] OF INT`. The declaration
    // constant of `Pulse`'s edge-phase member is the previous value of its R_EDGE input, FALSE.
    let prepared = prepare(BLOCKS);
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let pulse = InstanceId(1);
    assert_eq!(member(&state, pulse, "increment"), Value::Int(1));
    assert_eq!(member(&state, program, "toggle"), Value::Bool(false));
    assert_eq!(member(&state, program, "slot"), Value::Int(1));
    assert_eq!(
        member(&state, program, "counts"),
        array(vec![Value::Int(0), Value::Int(0)], vec![(0, 1)])
    );
    let pulse_pou = prepared.roots.entries[2]
        .template_pou_id
        .expect("the third root is the Pulse instance");
    let phases: Vec<&str> = prepared
        .instance_declarations(pulse_pou)
        .filter(|declaration| declaration.role == StorageRole::EdgePhase)
        .map(|declaration| &prepared.vm.strings[declaration.name_idx as usize])
        .collect();
    assert_eq!(phases.len(), 1);
    assert_eq!(member(&state, pulse, phases[0]), Value::Bool(false));
}

/// The structure `Holder` of `Keeper`: a reference `r` and a DINT `n`.
fn holder(reference: Option<ValueRef>, n: i32) -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("r"), Value::Reference(reference));
    fields.insert(SmolStr::new("n"), Value::DInt(n));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Holder"),
        fields,
    )))
}

/// The array `ARRAY[1..2] OF REF_TO DINT` of two references.
fn references(first: Option<ValueRef>, second: Option<ValueRef>) -> Value {
    array(
        vec![Value::Reference(first), Value::Reference(second)],
        vec![(1, 2)],
    )
}

#[test]
fn a_reference_in_a_structure_and_an_array_of_references_round_trip_on_cells() {
    // Specification 34 §6.2 (MEM-10; O9): a `REF_TO` member of a structure or an array, global or in an
    // instance, is a reference leaf reserved at construction, and it reads and writes through the region's
    // reference table. `Keeper` declares the globals `holder_g : Holder` (`r : REF_TO DINT; n : DINT`, the
    // global ordinal 1) and `slots_g : ARRAY[1..2] OF REF_TO DINT` (ordinal 2), and the function block
    // `Keeper` with `held : Holder` and `slots : ARRAY[1..2] OF REF_TO DINT`, whose instance is
    // `keeper_g`, identity 1 (`Main` takes 0). That is six reference leaves.
    //
    // Before any write every leaf is a null reference and `n` is 0. Then the structure `holder_g` takes a
    // reference to `holder_g.n` and 4, the array `slots_g` takes that reference and a null one, and one
    // element of the array is written through its path. The instance takes its two aggregates the same
    // way. Each read returns exactly what was written, the table's capacity never changes, and an INT
    // written where a reference belongs is refused and changes nothing. A store that refused a nested
    // reference or kept it outside the table would refuse these writes or read null.
    let prepared = keeper::prepared();
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let capacity = arm(&data).cells().regions().references().capacity();
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let keeper = InstanceId(1);
    assert_eq!(global(&state, "keeper_g"), Value::Instance(keeper));
    assert_eq!(global(&state, "holder_g"), holder(None, 0));
    assert_eq!(global(&state, "slots_g"), references(None, None));
    assert_eq!(member(&state, keeper, "held"), holder(None, 0));
    assert_eq!(member(&state, keeper, "slots"), references(None, None));

    let target = ValueRef {
        location: MemoryLocation::Global,
        offset: 1,
        generation: Generation::FIRST,
        path: vec![RefSegment::Field(SmolStr::new("n"))],
    };
    assert!(StoreMutate::write_slot(
        &mut state,
        MemoryLocation::Global,
        1,
        holder(Some(target.clone()), 4)
    ));
    assert_eq!(global(&state, "holder_g"), holder(Some(target.clone()), 4));
    assert!(StoreMutate::write_slot(
        &mut state,
        MemoryLocation::Global,
        2,
        references(Some(target.clone()), None)
    ));
    let second = [RefSegment::Index(vec![2])];
    let element = ValueRefView {
        location: MemoryLocation::Global,
        offset: 2,
        generation: Generation::FIRST,
        path: &second,
    };
    assert_eq!(
        StoreMutate::write_reference(&mut state, element, Value::Reference(Some(target.clone()))),
        Ok(true)
    );
    assert_eq!(
        global(&state, "slots_g"),
        references(Some(target.clone()), Some(target.clone()))
    );

    assert!(StoreMutate::write_instance_var(
        &mut state,
        keeper,
        "held",
        holder(Some(target.clone()), 7)
    ));
    assert!(StoreMutate::write_instance_var(
        &mut state,
        keeper,
        "slots",
        references(None, Some(target.clone()))
    ));
    assert_eq!(
        member(&state, keeper, "held"),
        holder(Some(target.clone()), 7)
    );
    assert_eq!(
        member(&state, keeper, "slots"),
        references(None, Some(target))
    );

    let member_r = [RefSegment::Field(SmolStr::new("r"))];
    let int_for_reference = ValueRefView {
        location: MemoryLocation::Global,
        offset: 1,
        generation: Generation::FIRST,
        path: &member_r,
    };
    assert!(StoreMutate::write_reference(&mut state, int_for_reference, Value::Int(3)).is_err());
    assert_eq!(
        capacity,
        arm(&data).cells().regions().references().capacity()
    );
}

#[test]
fn an_instance_without_a_root_is_placed_or_refused() {
    // Specification 34 §6.2 (MEM-10, MEM-12; O8): a persistent instance lives in its root's range.
    // `Keeper` has the roots `Main` (instance 0), the globals without instances, and `keeper_g`
    // (instance 1). The globals take 80 bytes (`holder_g` a 16-byte reference and a DINT, 20 bytes
    // rounded up to 24; `slots_g` two 16-byte references, 32; `keeper_g` an 8-byte instance leaf; the
    // two dates 8 each), so the roots start at byte 80: `Main` has no bytes of its own, and the
    // `Keeper` instance holds `held` (24) at 0 and `slots` (32) at 24, 56 bytes. Each root is placed in
    // its own range.
    //
    // An instance with no owner and no root has no range to live in. The cell store refuses it by name
    // and issues no identity, so the instance created next takes 2; the `Value` store has no range to
    // fill and keeps it, as it always did, and issues 2 for it. These are the paths that create such an
    // instance: the parent of an instance that no inheritance-parent root names, and a retained object
    // that no root of the replacement names. Both reach this reservation. The first is built end to end
    // below, from the cell fixture without its inheritance-parent root; the second calls the same
    // reservation (`RetainedGraph::instance`) and is not built, since no corpus program retains an
    // object that its replacement has no root for.
    let prepared = keeper::prepared();
    let mut cells = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    assert_eq!(
        arm(&cells).instance_range(InstanceId(0)),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: 80,
            bytes: 0,
        })
    );
    assert_eq!(
        arm(&cells).instance_range(InstanceId(1)),
        Some(ByteRange {
            region: LayoutRegion::Persistent,
            start: 80,
            bytes: 56,
        })
    );
    let mut state = EngineState::borrow(&mut cells, &prepared, &services::LOGICAL_ONLY);
    let refused =
        state.engineering_operation(|state| state.reserve_instance(keeper::KEEPER_POU, None));
    assert_eq!(
        refused,
        Err(RuntimeError::InitFailed {
            owner: SmolStr::new("Keeper"),
            variable: SmolStr::new("<instance>"),
            error: SmolStr::new(
                "a persistent instance without a construction root has no cell range"
            ),
        })
    );
    assert_eq!(
        StoreMutate::try_create_instance(&mut state, "Probe"),
        Ok(InstanceId(2))
    );

    let mut values = build(&prepared, StoreKind::Values).expect("the Value store constructs");
    let mut state = EngineState::borrow(&mut values, &prepared, &services::LOGICAL_ONLY);
    assert_eq!(
        state.engineering_operation(|state| state.reserve_instance(keeper::KEEPER_POU, None)),
        Ok(InstanceId(2))
    );

    // The parent of an instance: the cell fixture's `fb_g : Derived`, with the inheritance-parent root of
    // its instance removed, is a persistent root whose template extends `Base` and whose parent no root
    // names. The `Value` store creates that parent instance without a root, as it always did, and
    // constructs the module. The cell store has no range for it and refuses the construction by name,
    // before an identity is issued for the parent.
    let prepared = fixture::prepared_without_parent_root();
    assert!(build(&prepared, StoreKind::Values).is_ok());
    assert_eq!(
        build(&prepared, StoreKind::Cells).err(),
        Some(RuntimeError::InitFailed {
            owner: SmolStr::new("Base"),
            variable: SmolStr::new("<instance>"),
            error: SmolStr::new(
                "a persistent instance without a construction root has no cell range"
            ),
        })
    );
}

#[test]
fn a_cold_restart_reconstructs_the_store_it_replaces() {
    // Specification 34 §11.3 and TIME-09: a restart constructs the next generation on the store it
    // replaces, with the profile it ran with. A state built on cells holds cells after a cold restart, in
    // generation 1, and its program starts again from its initializers: `initializers` reads `slot` 1 and
    // the identities 0 and 1 again. A restart that rebuilt the `Value` store would leave the arm empty.
    let prepared = prepare(INITIALIZERS);
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    assert!(state.write_instance_var(InstanceId(0), "slot", Value::Int(3)));
    assert_eq!(member(&state, InstanceId(0), "slot"), Value::Int(3));
    state
        .restart(RestartMode::Cold)
        .expect("the cold restart constructs");
    assert!(state.holds_cells());
    assert_eq!(StoreAccess::generation(&state), Generation(1));
    assert_eq!(member(&state, InstanceId(0), "slot"), Value::Int(1));
    assert_eq!(global(&state, "Plant"), Value::Instance(InstanceId(0)));
    assert_eq!(
        member(&state, InstanceId(0), "tracker"),
        Value::Instance(InstanceId(1))
    );
}

#[test]
fn a_warm_restart_carries_the_retained_values_on_cells() {
    // Specification 11 §6.7 and IEC 61131-3 Ed.3 §6.5.6.1: a warm restart gives the RETAIN variables the
    // values they had when the resource stopped, and a cold restart their initial values; every other
    // variable starts again from its initializer. `retain` declares `VAR_GLOBAL RETAIN counter : DINT :=
    // 0; setpoint : REAL := 20.0;`, `VAR_GLOBAL ticks : DINT := 0;` and, in the program (instance 0),
    // `VAR RETAIN runs : DINT; label : STRING[12] := 'boiler';`. The run changes all five to 7, 21.5, 9,
    // 3 and 'cooked'. After the warm restart `counter`, `setpoint`, `runs` and `label` read 7, 21.5, 3
    // and 'cooked' and `ticks` reads 0 again; after the cold restart that follows, all five read their
    // initial values 0, 20.0, 0 and 'boiler', and `ticks` 0. A restart that read the retained values from
    // the `Value` half would carry nothing, and one that kept every value would carry `ticks` too.
    let prepared = prepare(RETAIN);
    let mut data = build(&prepared, StoreKind::Cells).expect("the cell store constructs");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let program = InstanceId(0);
    assert!(StoreMutate::set_global(
        &mut state,
        "counter",
        Value::DInt(7)
    ));
    assert!(StoreMutate::set_global(
        &mut state,
        "setpoint",
        Value::Real(21.5)
    ));
    assert!(StoreMutate::set_global(&mut state, "ticks", Value::DInt(9)));
    assert!(state.write_instance_var(program, "runs", Value::DInt(3)));
    assert!(state.write_instance_var(program, "label", Value::String(SmolStr::new("cooked"))));

    state
        .restart(RestartMode::Warm)
        .expect("the warm restart constructs");
    assert!(state.holds_cells());
    assert_eq!(global(&state, "counter"), Value::DInt(7));
    assert_eq!(global(&state, "setpoint"), Value::Real(21.5));
    assert_eq!(global(&state, "ticks"), Value::DInt(0));
    assert_eq!(member(&state, program, "runs"), Value::DInt(3));
    assert_eq!(
        member(&state, program, "label"),
        Value::String(SmolStr::new("cooked"))
    );

    state
        .restart(RestartMode::Cold)
        .expect("the cold restart constructs");
    assert_eq!(global(&state, "counter"), Value::DInt(0));
    assert_eq!(global(&state, "setpoint"), Value::Real(20.0));
    assert_eq!(global(&state, "ticks"), Value::DInt(0));
    assert_eq!(member(&state, program, "runs"), Value::DInt(0));
    assert_eq!(
        member(&state, program, "label"),
        Value::String(SmolStr::new("boiler"))
    );
}
