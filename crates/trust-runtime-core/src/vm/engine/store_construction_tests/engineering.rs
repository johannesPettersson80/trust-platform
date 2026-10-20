//! The engine's engineering and host paths read and write the persistent state through the store-access
//! seam (specification 34 §6.2, MEM-10; C1-9d3a, R1; C1-19e).
//!
//! A scan reaches the program root, the input apply, the output journals and the built-in blocks (the
//! corpus runs in `scans.rs` cover them). These tests reach the rest of the paths that the store-access
//! seam carries: the engineering reads and writes of `access.rs`, the lookups of the cycle port, the
//! scalar readers of `RuntimeState`, the entry of a named program, the retained-snapshot apply and the key
//! and the restore of an output journal. The expected values are worked out by hand from the ST source of
//! the corpus program each test names. Until the `Value` store was deleted, each test also ran on it and
//! required the same work and allocation from both stores; those charges are recorded (`expected.rs`),
//! and the flat store's are compared with them.

use super::super::destinations::SavedDestinations;
use super::expected::Facts;
use super::{array, build, corpus, declared_values, global, member, services, EngineState};
use crate::error::RuntimeError;
use crate::memory::view::{StoreAccess, StoreMutate};
use crate::memory::{Generation, InstanceId, MemoryLocation};
use crate::retain::RetainSnapshot;
use crate::value::{Duration, RefSegment, StructValue, Value, ValueRef};
use crate::vm::{CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule};
use crate::vm::{RetainApplyEvent, RuntimeState};
use smol_str::SmolStr;
use std::sync::Arc;

/// The corpus program `name`, prepared under the default limits.
fn prepare_corpus(name: &str) -> PreparedModule {
    PreparedModule::from_bytes(corpus(name), PreparationLimits::default())
        .expect("the program prepares")
}

/// Run `operation` as an engineering operation, check its result, the work left in its allowance and
/// the allocation bytes it charged (TIME-08) under `key`, and answer its result.
fn charged<T: core::fmt::Debug>(
    state: &mut EngineState<'_>,
    facts: &mut Facts,
    key: &str,
    operation: impl Fn(&mut EngineState<'_>) -> Result<T, RuntimeError>,
) -> Result<T, RuntimeError> {
    let result = state.engineering_operation(operation);
    let resources = &state.data.resources;
    facts.check(
        key,
        &[
            format!("{result:?}"),
            format!("work left {}", resources.work_budget.remaining()),
            format!("bytes {}", resources.construction_bytes.get()),
        ],
    );
    result
}

/// The reference to the global `name`.
fn reference_to(state: &EngineState<'_>, name: &str) -> ValueRef {
    StoreAccess::ref_for_global(state, name).unwrap_or_else(|| panic!("{name} is declared"))
}

/// The reads and writes of `references`.
fn engineering_reads_and_writes(facts: &mut Facts) {
    let prepared = prepare_corpus("references");
    let mut data = build(&prepared).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let target = reference_to(&state, "target");
    let key = |label: &str| format!("engineering/references/{label}");

    let read =
        |state: &mut EngineState<'_>, facts: &mut Facts, label: &str, reference: &ValueRef| {
            charged(state, facts, &key(label), |state| {
                state.read_checked_reference(reference)
            })
        };
    // `target : DINT := DINT#5`.
    assert_eq!(
        read(&mut state, facts, "read target", &target),
        Ok(Value::DInt(5))
    );

    // A whole-global write lands in the store and reads back; a value of another type is refused and
    // changes nothing.
    assert_eq!(
        charged(&mut state, facts, &key("write target"), |state| {
            state.write_global("target", Value::DInt(41))
        }),
        Ok(())
    );
    assert_eq!(
        read(&mut state, facts, "read target after the write", &target),
        Ok(Value::DInt(41))
    );
    assert!(charged(&mut state, facts, &key("refused type"), |state| {
        state.write_global("target", Value::Bool(true))
    })
    .is_err());
    assert_eq!(
        read(&mut state, facts, "read target after the refusal", &target),
        Ok(Value::DInt(41))
    );

    // An element of an array global: `store[2]` of `ARRAY[1..4] OF DINT` is its second element.
    let store = reference_to(&state, "store");
    assert_eq!(
        charged(&mut state, facts, &key("write store[2]"), |state| {
            state.write_named_path("store", &[RefSegment::Index(vec![2])], Value::DInt(7))
        }),
        Ok(())
    );
    let written = array(
        vec![
            Value::DInt(0),
            Value::DInt(7),
            Value::DInt(0),
            Value::DInt(0),
        ],
        vec![(1, 4)],
    );
    assert_eq!(
        read(&mut state, facts, "read store", &store),
        Ok(written.clone())
    );
    let mut element = store.clone();
    element.path.push(RefSegment::Index(vec![2]));
    assert_eq!(
        read(&mut state, facts, "read store[2]", &element),
        Ok(Value::DInt(7))
    );
    // Index 5 is outside the declared bounds 1..4: the write is refused and no element changes.
    assert!(charged(&mut state, facts, &key("store[5]"), |state| {
        state.write_named_path("store", &[RefSegment::Index(vec![5])], Value::DInt(1))
    })
    .is_err());
    assert_eq!(
        read(&mut state, facts, "read store after the refusal", &store),
        Ok(written)
    );

    // A member of a structure global.
    let reading = reference_to(&state, "reading");
    assert_eq!(
        charged(&mut state, facts, &key("write reading.amount"), |state| {
            state.write_named_path(
                "reading",
                &[RefSegment::Field(SmolStr::new("amount"))],
                Value::DInt(9),
            )
        }),
        Ok(())
    );
    let mut fields = crate::collections::OrderedMap::default();
    fields.insert(SmolStr::new("amount"), Value::DInt(9));
    fields.insert(SmolStr::new("valid"), Value::Bool(false));
    assert_eq!(
        read(&mut state, facts, "read reading", &reading),
        Ok(Value::Struct(Arc::new(StructValue::from_canonical_parts(
            SmolStr::new("Sample"),
            fields
        ))))
    );

    // A global that is not declared names nothing, and a program root is not replaced.
    assert_eq!(
        charged(&mut state, facts, &key("unknown"), |state| {
            state.write_global("nothing_declared", Value::DInt(1))
        }),
        Err(RuntimeError::NullReference)
    );
    assert_eq!(
        charged(&mut state, facts, &key("root"), |state| {
            state.write_global("Plant", Value::Instance(InstanceId(0)))
        }),
        Err(RuntimeError::ProgramRootReplacement)
    );

    // A reference is checked against the store's own generation (specification 34 §11.3, MEM-07): a
    // handle of another generation is refused with `ReferenceLifetime`, for a read and for a write, and the
    // current handle writes through.
    let stale = ValueRef {
        generation: Generation(7),
        ..target.clone()
    };
    assert_eq!(
        read(&mut state, facts, "stale read", &stale),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        charged(&mut state, facts, &key("stale write"), |state| {
            state.write_checked_reference(stale.as_view(), Value::DInt(1))
        }),
        Err(RuntimeError::ReferenceLifetime)
    );
    assert_eq!(
        charged(&mut state, facts, &key("reference write"), |state| {
            state.write_checked_reference(target.as_view(), Value::DInt(42))
        }),
        Ok(())
    );
    assert_eq!(
        read(
            &mut state,
            facts,
            "read target after the reference write",
            &target
        ),
        Ok(Value::DInt(42))
    );

    // Everything the program holds is as the `Value` store held it.
    facts.check(&key("declared values"), &declared_values(&prepared, &state));
}

#[test]
fn engineering_reads_and_writes_resolve_check_and_charge_as_the_value_store_did() {
    // Specification 34 §6.2 (MEM-10) and specification 11 (engineering access): an engineering read or
    // write of a global resolves, checks and charges as it did on the `Value` store. The program is
    // `references` (`VAR_GLOBAL ticks : DINT := 0; store : ARRAY[1..4] OF DINT; reading : Sample; target :
    // DINT := 5`, with `Sample` a structure of `amount : DINT` and `valid : BOOL`; the program instance
    // `Plant`).
    //
    // A store that wrote an engineering value into the `Value` half would read the old value back; one
    // that checked a path against the wrong layout would accept index 5 of `ARRAY[1..4]`; one that
    // compared a reference's generation with a store's other counter would accept a stale handle.
    let mut facts = Facts::compared();
    engineering_reads_and_writes(&mut facts);
    facts.finish("engineering/references/");
}

/// The characters of `strings`' `name`.
fn character_reads(facts: &mut Facts) {
    let prepared = prepare_corpus("strings");
    let mut data = build(&prepared).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let program = InstanceId(0);
    let name = StoreAccess::ref_for_instance(&state, program, "name").expect("name is declared");
    let mut first = name.clone();
    first.path.push(RefSegment::Index(vec![1]));
    assert_eq!(
        charged(&mut state, facts, "engineering/strings/name[1]", |state| {
            state.read_checked_reference(&first)
        }),
        Ok(Value::Char(b'p'))
    );
    let mut beyond = name;
    beyond.path.push(RefSegment::Index(vec![17]));
    assert!(
        charged(&mut state, facts, "engineering/strings/name[17]", |state| {
            state.read_checked_reference(&beyond)
        })
        .is_err()
    );
}

#[test]
fn a_character_reference_reads_and_charges_as_the_value_store_did() {
    // Specification 11 and IEC 61131-3 Ed.3 §6.4.2.2: the first character of a `STRING` is index 1. The
    // program is `strings`, whose program instance holds `name : STRING[16] := 'pump'`; the character at
    // index 1 is `p` (112). A character has no stored value, so the engineering read resolves its parent
    // text and charges the scan of the text (TIME-08), as the `Value` store charged it. Index 17 lies
    // beyond the text's 16 characters and is refused. A store that looked for the element in the `Value`
    // half would find no instance.
    let mut facts = Facts::compared();
    character_reads(&mut facts);
    facts.finish("engineering/strings/");
}

/// What the cycle port answers about `references` at the start of a cycle.
#[derive(Debug, Default, PartialEq)]
struct Lookups {
    generation: Option<Generation>,
    target_generation: Option<Generation>,
    target: Option<Value>,
    slot: Option<Value>,
    recursive_is_direct: Option<bool>,
    plant: Option<Option<InstanceId>>,
    target_instance: Option<Option<InstanceId>>,
    unknown: (Option<ValueRef>, Option<ValueRef>, Option<InstanceId>),
}

impl CycleHooks for Lookups {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        if phase != CyclePhase::CycleStart {
            return Ok(());
        }
        let program = InstanceId(0);
        self.generation = Some(access.generation());
        let target = access.ref_for_global("target").expect("target is declared");
        self.target_generation = Some(target.generation);
        self.target = Some(access.read_reference(&target)?);
        let slot = access
            .ref_for_instance(program, "slot")
            .expect("slot is declared");
        self.slot = Some(access.read_reference(&slot)?);
        self.recursive_is_direct =
            Some(access.ref_for_instance_recursive(program, "slot") == Some(slot));
        self.plant = Some(access.global_instance("Plant"));
        self.target_instance = Some(access.global_instance("target"));
        self.unknown = (
            access.ref_for_global("nothing_declared"),
            access.ref_for_instance(program, "nothing_declared"),
            access.global_instance("nothing_declared"),
        );
        Ok(())
    }
}

#[test]
fn cycle_port_lookups_resolve_in_the_store() {
    // Specification 34 §4.3.1 and §11.3: the cycle port resolves a declared global or member to a reference
    // of the installation's generation, and the instance a global holds. The program is `references`
    // (`target : DINT := 5` global; the program instance `Plant`, identity 0, with `slot : DINT := 1`).
    // A store that resolved the lookups in the `Value` half would find no `target` and no `Plant`; one
    // that stamped the references with another generation would have them refused when read back.
    let prepared = prepare_corpus("references");
    let mut data = build(&prepared).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let mut lookups = Lookups::default();
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut lookups)
        .unwrap_or_else(|error| panic!("the cycle ends with {error:?}"));
    assert_eq!(
        lookups,
        Lookups {
            generation: Some(Generation::FIRST),
            target_generation: Some(Generation::FIRST),
            target: Some(Value::DInt(5)),
            slot: Some(Value::DInt(1)),
            recursive_is_direct: Some(true),
            plant: Some(Some(InstanceId(0))),
            target_instance: Some(None),
            unknown: (None, None, None),
        }
    );
}

#[test]
fn runtime_state_scalar_readers_answer_from_the_store() {
    // Specification 34 §6.2 and specification 02 §1: the public readers of a state answer from the store it
    // holds. The program is `initializers`: `ticks : DINT := 0`, the constant `capacity : INT := 9`, the
    // array `table`, the structure `origin`, the program instance `Plant` (identity 0) with the constant
    // `gain : INT := 2`, `slot : INT := 1`, `result : INT` (0) and `tracker : Tracker`, whose instance is
    // identity 1 and holds the array `history`. A scalar is read by value; any other value is refused with
    // `TypeMismatch`, never read as a default; a name or an instance that does not exist reads `None`.
    let prepared = prepare_corpus("initializers");
    let state = prepared
        .instantiate_with_services(0, &services::LOGICAL_ONLY)
        .expect("the store instantiates");
    check_scalar_readers(&state, InstanceId(0), InstanceId(1));
}

fn check_scalar_readers(state: &RuntimeState<'_>, program: InstanceId, tracker: InstanceId) {
    assert_eq!(state.read_global_scalar("ticks"), Ok(Some(Value::DInt(0))));
    assert_eq!(
        state.read_global_scalar("capacity"),
        Ok(Some(Value::Int(9)))
    );
    for aggregate in ["table", "origin", "Plant"] {
        assert_eq!(
            state.read_global_scalar(aggregate),
            Err(RuntimeError::TypeMismatch),
            "{aggregate}"
        );
    }
    assert_eq!(state.read_global_scalar("nothing_declared"), Ok(None));

    for (name, expected) in [("slot", 1), ("result", 0), ("gain", 2)] {
        assert_eq!(
            state.read_instance_scalar(program, name),
            Ok(Some(Value::Int(expected))),
            "{name}"
        );
    }
    assert_eq!(
        state.read_instance_scalar(program, "tracker"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.read_instance_scalar(program, "nothing_declared"),
        Ok(None)
    );
    assert_eq!(state.read_instance_scalar(InstanceId(99), "slot"), Ok(None));

    assert_eq!(state.read_global_instance("Plant"), Ok(Some(program)));
    assert_eq!(
        state.read_global_instance("ticks"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(state.read_global_instance("nothing_declared"), Ok(None));
    assert_eq!(
        state.read_instance_member_instance(program, "tracker"),
        Ok(Some(tracker))
    );
    assert_eq!(
        state.read_instance_member_instance(program, "slot"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.read_instance_member_instance(tracker, "history"),
        Err(RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.read_instance_member_instance(InstanceId(99), "tracker"),
        Ok(None)
    );
}

/// The named entry of `scalar`'s program.
fn named_program_entry(facts: &mut Facts) {
    let prepared = prepare_corpus("scalar");
    let mut data = build(&prepared).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    assert_eq!(
        charged(&mut state, facts, "engineering/scalar/execute", |state| {
            state.execute_named_program("Plant")
        }),
        Ok(())
    );
    assert_eq!(global(&state, "ticks"), Value::DInt(1));
    assert_eq!(global(&state, "total"), Value::LInt(7));
    assert_eq!(global(&state, "level"), Value::LReal(2.0));
    let program = InstanceId(0);
    assert_eq!(member(&state, program, "sum"), Value::DInt(104));
    assert_eq!(member(&state, program, "phase"), Value::Int(1));
    assert_eq!(member(&state, program, "ready"), Value::Bool(true));
    // A name that is no program root is no program, whatever it names.
    for name in ["nothing_declared", "ticks"] {
        assert_eq!(
            charged(
                &mut state,
                facts,
                &format!("engineering/scalar/{name}"),
                |state| state.execute_named_program(name)
            ),
            Err(RuntimeError::UndefinedProgram(name.into())),
        );
    }
    facts.check(
        "engineering/scalar/declared values",
        &declared_values(&prepared, &state),
    );
}

#[test]
fn a_named_program_entry_runs_the_program() {
    // Specification 11 (explicit program entry): `execute_program` runs the program a root names, once,
    // with the allowance of an engineering operation. The program is `scalar`, whose one run from its
    // initial values leaves `ticks` 1, `total` 7, `sum` 104 (the sum over 1..8 of 3 × index less 1 for
    // each odd index: 3 × 36 - 4), `phase` 1 (the CASE moves 0 to 1), `ready` TRUE (`sum > 50` and not yet
    // ready) and `level` 2.0 (0.0 × 0.5 + 2.0). The entry is charged as the `Value` store charged it. A
    // store that read the root from the `Value` half would find no instance and refuse the entry.
    let mut facts = Facts::compared();
    named_program_entry(&mut facts);
    facts.finish("engineering/scalar/");
}

/// The retained snapshot applied to `retain`.
fn retained_snapshot_apply(facts: &mut Facts) {
    let prepared = prepare_corpus("retain");
    let mut data = build(&prepared).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &prepared, &services::LOGICAL_ONLY);
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("counter", Value::DInt(5));
    snapshot.insert("@program/Plant/runs", Value::Int(7));
    snapshot.insert("gone", Value::DInt(1));
    snapshot.insert("setpoint", Value::Real(21.5));
    snapshot.insert(
        "@program/Plant/label",
        Value::String(SmolStr::new("cooked")),
    );
    let events = charged(&mut state, facts, "engineering/retain/apply", |state| {
        state.apply_retained_snapshot(&snapshot)
    });
    assert_eq!(
        events,
        Ok(vec![
            RetainApplyEvent::Migrated {
                name: SmolStr::new("@program/Plant/runs"),
                before: Value::Int(7),
                after: Value::DInt(7),
            },
            RetainApplyEvent::Orphan {
                name: SmolStr::new("gone"),
            },
        ])
    );
    let program = InstanceId(0);
    assert_eq!(global(&state, "counter"), Value::DInt(5));
    assert_eq!(global(&state, "setpoint"), Value::Real(21.5));
    assert_eq!(global(&state, "ticks"), Value::DInt(0));
    assert_eq!(member(&state, program, "runs"), Value::DInt(7));
    assert_eq!(
        member(&state, program, "label"),
        Value::String(SmolStr::new("cooked"))
    );
    facts.check(
        "engineering/retain/declared values",
        &declared_values(&prepared, &state),
    );
}

#[test]
fn a_retained_snapshot_applies_to_the_variables_it_names() {
    // Specification 11 §6.7 (retained-value migration) and IEC 61131-3 Ed.3 §6.5.6.1: a snapshot is applied
    // to the retained variables it names, by their saved keys, converted to the admitted type; a key that
    // names nothing is an orphan. The program is `retain`: `VAR_GLOBAL RETAIN counter : DINT := 0;
    // setpoint : REAL := 20.0`, `ticks : DINT := 0` (not retained) and, in the program `Plant`, `VAR RETAIN
    // runs : DINT; label : STRING[12] := 'boiler'`. The snapshot holds `counter` 5, `Plant.runs` as the
    // INT 7, `gone` (no such key), `setpoint` 21.5 and `Plant.label` 'cooked'. Applied, `counter` is 5,
    // `setpoint` 21.5, `runs` the DINT 7 (the one conversion, reported as an event, in snapshot order
    // before the orphan), `label` 'cooked' and `ticks` still 0, charged as the `Value` store charged it. A
    // store that read the retained variables from the `Value` half would find no variable to journal and
    // refuse the whole snapshot.
    let mut facts = Facts::compared();
    retained_snapshot_apply(&mut facts);
    facts.finish("engineering/retain/");
}

#[test]
fn an_output_journal_follows_instance_hops_and_restores_aggregates() {
    // Specification 34 TIME-08 and MEM-09 (call transactions): a call that copies its outputs back journals
    // the physical slot each destination lies in, so a later failure restores every one of them. The key of
    // a destination is the slot that holds it once the instance hops of its path are followed.
    //
    // `blocks`: the program instance (identity 0) holds `pulse : Pulse`, whose instance is identity 1; the
    // destination `pulse.count` is member `count` of that instance, so its key is that instance and the
    // member's slot, which a lookup of the member names, and the root saved there is the INT 0, saved as
    // its two zero bytes (C1-16). `arrays`: the program holds `data : ARRAY[1..8] OF DINT`, zero at the
    // start; the destination `data[3]` lies in the slot of `data`, and the root saved is the whole array,
    // its 32 zero bytes. Writing 77 into the element and restoring the journal puts the whole array back.
    // A store that did not follow the hop would journal the slot of `pulse` itself, and one that restored
    // into the `Value` half would leave the cells changed.
    let blocks = prepare_corpus("blocks");
    let mut data = build(&blocks).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &blocks, &services::LOGICAL_ONLY);
    let (program, pulse) = (InstanceId(0), InstanceId(1));
    let pulse_slot = StoreAccess::ref_for_instance(&state, program, "pulse").expect("pulse");
    let count_slot = StoreAccess::ref_for_instance(&state, pulse, "count").expect("count");
    let mut count = pulse_slot;
    count.path.push(RefSegment::Field(SmolStr::new("count")));
    let key = (MemoryLocation::Instance(pulse), count_slot.offset as usize);
    let mut saved = SavedDestinations::new();
    state
        .snapshot_destinations(None, std::iter::once(count.as_view()), &mut saved)
        .unwrap_or_else(|error| panic!("the destination is journaled: {error:?}"));
    assert_eq!(saved.keys().copied().collect::<Vec<_>>(), [key]);
    assert_eq!(saved.saved_bytes(&key), Some(&[0u8, 0][..]));
    assert!(state.write_instance_var(pulse, "count", Value::Int(5)));
    assert_eq!(member(&state, pulse, "count"), Value::Int(5));
    state.restore_destinations(None, &mut saved);
    assert_eq!(member(&state, pulse, "count"), Value::Int(0));
    assert_eq!(saved.len(), 0);

    let arrays = prepare_corpus("arrays");
    let mut data = build(&arrays).expect("the store constructs the program");
    let mut state = EngineState::borrow(&mut data, &arrays, &services::LOGICAL_ONLY);
    let data_slot = StoreAccess::ref_for_instance(&state, program, "data").expect("data");
    let key = (MemoryLocation::Instance(program), data_slot.offset as usize);
    let mut element = data_slot;
    element.path.push(RefSegment::Index(vec![3]));
    let zeros = array(vec![Value::DInt(0); 8], vec![(1, 8)]);
    let mut changed = vec![Value::DInt(0); 8];
    changed[2] = Value::DInt(77);
    let changed = array(changed, vec![(1, 8)]);
    let mut saved = SavedDestinations::new();
    state
        .snapshot_destinations(None, std::iter::once(element.as_view()), &mut saved)
        .unwrap_or_else(|error| panic!("the destination is journaled: {error:?}"));
    assert_eq!(saved.keys().copied().collect::<Vec<_>>(), [key]);
    assert_eq!(saved.saved_bytes(&key), Some(&[0u8; 32][..]));
    assert_eq!(
        state.write_reference(element.as_view(), Value::DInt(77)),
        Ok(true)
    );
    assert_eq!(member(&state, program, "data"), changed);
    state.restore_destinations(None, &mut saved);
    assert_eq!(member(&state, program, "data"), zeros);
}

/// Record the facts of every test of this file.
pub(super) fn record(facts: &mut Facts) {
    engineering_reads_and_writes(facts);
    character_reads(facts);
    named_program_entry(facts);
    retained_snapshot_apply(facts);
}
