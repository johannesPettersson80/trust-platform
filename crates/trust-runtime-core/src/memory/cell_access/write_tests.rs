//! Tests of the cell arm's writes (specification 34 §6.2, MEM-10; C1-9c). They write the same program
//! into the cell arm and into the `Value` store and compare the reads, and each expected value is worked
//! out by hand from the ST source in `super::tests`:
//!
//! ```text
//! TYPE Pair : STRUCT x : INT; y : INT; END_STRUCT END_TYPE
//! FUNCTION_BLOCK Base VAR base_v : DINT; series_v : ARRAY[1..3] OF INT; pair_v : Pair; END_VAR
//! FUNCTION_BLOCK Derived EXTENDS Base VAR own_v : REAL; END_VAR
//! VAR_GLOBAL dint_g : DINT; real_g : REAL; text_g : STRING[10]; wide_g : WSTRING[5];
//!            base_g : Base; fb_g : Derived; END_VAR
//! ```
//!
//! The initial values are `dint_g` 7, `real_g` 1.5, `text_g` 'abc', `wide_g` 'hi', `base_g.base_v` 5,
//! `base_g.series_v` 1, 2, 3, `base_g.pair_v` (10, 20), `fb_g.own_v` 2.5 and the 5, 1, 2, 3, (10, 20) of
//! the instance `fb_g` inherits.

use std::sync::Arc;

use smol_str::SmolStr;

use super::cell_charge::CopyCharge;
use super::fixture::{
    prepared_with_holder, BASE_G, BASE_G_INSTANCE, DERIVED_INSTANCE, DINT_G, FB_G, GLOBAL_NAMES,
    HOLDER_G, REF_G,
};
use super::tests::{field_segment, value_of, Fixture};
use super::{CellArm, CellPatch, CellWriter};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::regions::CellStorage;
use crate::memory::view::{PreparedWriteKind, StoreAccess, StoreMutate, ValueView};
use crate::memory::{Generation, MemoryLocation, VariableStorage};
use crate::value::{RefSegment, StructValue, Value, ValueRef, ValueRefView, MAX_STRING_CAPACITY};

/// The `GLOBAL_NAMES` ordinals of the globals the tests write.
const REAL_G: usize = 2;
const TEXT_G: usize = 3;
const WIDE_G: usize = 4;
const MAIN_G: usize = 0;

/// Run `write` on a writer of the cell arm of `fixture`.
fn with_writer<T>(fixture: &mut Fixture, write: impl FnOnce(&mut CellWriter<'_>) -> T) -> T {
    let mut writer = CellWriter::new(&fixture.prepared, &mut fixture.arm, &mut fixture.half);
    write(&mut writer)
}

/// The cells of a cell view's copy charge, computed from the view (TIME-08; C1-9c, R5).
fn charge_of(view: ValueView<'_>) -> CopyCharge {
    match view {
        ValueView::Cells {
            cells,
            range,
            type_id,
        } => cells
            .copy_charge(range, type_id, 0)
            .expect("the cells of a value copy"),
        ValueView::Value(_) | ValueView::Bits { .. } | ValueView::Instance(_) => {
            panic!("a view of a declaration of the cell arm holds cells")
        }
    }
}

#[test]
fn write_normalizes_to_the_declared_type() {
    // Specification 34 §6.2 (O11, R2): a write is widened by the declared type, never by the stored
    // variant. A SINT written to `dint_g` (DINT) reads back as DINT 3, and an INT written to `real_g`
    // (REAL) as REAL 2.0. The `Value` store widens by the variant it holds (DInt, Real) and gives the
    // same. A cell arm that stored the SINT unwidened would refuse it, since a DINT cell holds no SINT.
    let mut fixture = Fixture::new();
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, DINT_G, Value::SInt(3))
    }));
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, REAL_G, Value::Int(2))
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        DINT_G,
        Value::SInt(3)
    ));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        REAL_G,
        Value::Int(2)
    ));
    assert_eq!(
        fixture.reader().read_global("dint_g").map(value_of),
        Some(Value::DInt(3))
    );
    assert_eq!(
        fixture.reader().read_global("real_g").map(value_of),
        Some(Value::Real(2.0))
    );
    assert_eq!(
        StoreAccess::read_global(&fixture.values, "dint_g").map(value_of),
        Some(Value::DInt(3))
    );
    assert_eq!(
        StoreAccess::read_global(&fixture.values, "real_g").map(value_of),
        Some(Value::Real(2.0))
    );
}

#[test]
fn a_null_slot_is_not_widened() {
    // Specification 34 §6.2 (O11): a slot that holds Null is not widened, as the `Value` store kept it
    // (`value/types.rs:943`, the last arm). The program root `Main` holds no storage on the flat store
    // (specification 12 §6.4, construction-only; C1-19e): it names its program's instance, which the
    // fixture does not construct, so it reads as nothing and a write to it is refused, where the `Value`
    // store kept a SINT 3 written over its Null. A Null written over the cell-backed `dint_g` leaves the
    // empty cell, which reads as Null and not as the DINT zero.
    let mut fixture = Fixture::new();
    assert_eq!(fixture.reader().read_global("Main").map(value_of), None);
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, MAIN_G, Value::SInt(3))
    }));
    assert_eq!(fixture.reader().read_global("Main").map(value_of), None);
    // A DINT holds four bytes and no tag (specification 34 MEM-12): the null value writes zero
    // bytes, which read as 0.
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, DINT_G, Value::Null)
    }));
    assert_eq!(
        fixture.reader().read_global("dint_g").map(value_of),
        Some(Value::DInt(0))
    );
}

#[test]
fn refused_write_leaves_every_cell_unchanged() {
    // Specification 34 §6.2 (R4): a refused write copies nothing. A `Pair` whose `x` is a valid INT and
    // whose `y` is a BOOL is refused, because `y` does not match INT. A writer that copied `x` before it
    // met the refusal of `y` would leave `x` at 1, so the test reads both members back: `base_g.pair_v`
    // still holds x 10 and y 20.
    let mut fixture = Fixture::new();
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("x"), Value::Int(1));
    fields.insert(SmolStr::new("y"), Value::Bool(true));
    let refused = Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Pair"),
        fields,
    )));
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::write_instance_var(writer, BASE_G_INSTANCE, "pair_v", refused.clone())
    }));
    assert!(fixture
        .reader()
        .prepare_write(
            MemoryLocation::Global,
            BASE_G,
            &[field_segment("pair_v")],
            &refused
        )
        .is_err());
    assert_eq!(
        fixture
            .reader()
            .read_path(
                MemoryLocation::Global,
                BASE_G,
                &[field_segment("pair_v"), field_segment("x")]
            )
            .ok()
            .map(value_of),
        Some(Value::Int(10))
    );
    assert_eq!(
        fixture
            .reader()
            .read_path(
                MemoryLocation::Global,
                BASE_G,
                &[field_segment("pair_v"), field_segment("y")]
            )
            .ok()
            .map(value_of),
        Some(Value::Int(20))
    );
}

#[test]
fn a_text_write_is_cut_at_its_capacity() {
    // Specification 34 §6.2 (R3) and IEC 61131-3 Ed.3 §6.4.2.2: a text is cut to its declared capacity.
    // `text_g` is STRING[10], so the 15 characters 'abcdefghijklmno' keep their first 10. `wide_g` is
    // WSTRING[5], so 'abcdefg' keeps its first 5 UTF-16 code units. A cell arm that did not cut would
    // store all 15 and all 7.
    let mut fixture = Fixture::new();
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(
            writer,
            MemoryLocation::Global,
            TEXT_G,
            Value::String(SmolStr::new("abcdefghijklmno")),
        )
    }));
    assert_eq!(
        fixture.reader().read_global("text_g").map(value_of),
        Some(Value::String(SmolStr::new("abcdefghij")))
    );
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(
            writer,
            MemoryLocation::Global,
            WIDE_G,
            Value::WString(String::from("abcdefg")),
        )
    }));
    assert_eq!(
        fixture.reader().read_global("wide_g").map(value_of),
        Some(Value::WString(String::from("abcde")))
    );
}

/// Specification 34 §6.2 and §3.1 of the C1-9 note: the same writes to the cell arm and to the `Value`
/// store give the same reads. Each written value has its target's declared type, so both stores keep it
/// as written. The expected values, worked out by hand, are: dint_g 3 (written 4, then the INT 3 by
/// reference, widened to DINT), real_g 2.5, text_g 'xyz', base_g.base_v 9, base_g.series_v[2] 7,
/// base_g.pair_v.y 33, fb_g.own_v 1.5, and the parent's base_v 5, which `base_g` does not change.
#[test]
fn cell_writes_then_reads_equal_value_writes_then_reads() {
    let mut fixture = Fixture::new();
    apply_cell_and_value_writes(&mut fixture);
    assert_cell_and_value_reads_equal(&fixture);
}

/// The same writes applied to the cell arm and to the `Value` store of `fixture`.
#[allow(
    clippy::too_many_lines,
    reason = "the writes of both stores, one statement each, in the order the test documents"
)]
fn apply_cell_and_value_writes(fixture: &mut Fixture) {
    assert!(with_writer(fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, DINT_G, Value::DInt(4))
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        DINT_G,
        Value::DInt(4)
    ));
    assert!(with_writer(fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, REAL_G, Value::Real(2.5))
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        REAL_G,
        Value::Real(2.5)
    ));
    assert!(with_writer(fixture, |writer| {
        StoreMutate::write_slot(
            writer,
            MemoryLocation::Global,
            TEXT_G,
            Value::String(SmolStr::new("xyz")),
        )
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        TEXT_G,
        Value::String(SmolStr::new("xyz"))
    ));
    assert!(with_writer(fixture, |writer| {
        StoreMutate::write_instance_var(writer, BASE_G_INSTANCE, "base_v", Value::DInt(9))
    }));
    assert!(StoreMutate::write_instance_var(
        &mut fixture.values,
        BASE_G_INSTANCE,
        "base_v",
        Value::DInt(9)
    ));
    assert!(with_writer(fixture, |writer| {
        StoreMutate::write_instance_var(writer, DERIVED_INSTANCE, "own_v", Value::Real(1.5))
    }));
    assert!(StoreMutate::write_instance_var(
        &mut fixture.values,
        DERIVED_INSTANCE,
        "own_v",
        Value::Real(1.5)
    ));

    // Element and member writes, prepared on each store and committed once.
    let element = [field_segment("series_v"), RefSegment::Index(vec![2])];
    let cell_prepared = fixture
        .reader()
        .prepare_write(MemoryLocation::Global, BASE_G, &element, &Value::Int(7))
        .expect("the element prepares")
        .expect("the element is a target");
    assert!(with_writer(fixture, |writer| {
        StoreMutate::commit_write(
            writer,
            &element,
            Value::Int(7),
            cell_prepared,
            MAX_STRING_CAPACITY,
        )
    }));
    let value_prepared = StoreAccess::prepare_write(
        &fixture.values,
        MemoryLocation::Global,
        BASE_G,
        &element,
        &Value::Int(7),
    )
    .expect("the element prepares in the value store")
    .expect("the element is a target in the value store");
    assert!(StoreMutate::commit_write(
        &mut fixture.values,
        &element,
        Value::Int(7),
        value_prepared,
        MAX_STRING_CAPACITY
    ));

    let member = [field_segment("pair_v"), field_segment("y")];
    let cell_prepared = fixture
        .reader()
        .prepare_write(MemoryLocation::Global, BASE_G, &member, &Value::Int(33))
        .expect("the member prepares")
        .expect("the member is a target");
    assert!(with_writer(fixture, |writer| {
        StoreMutate::commit_write(
            writer,
            &member,
            Value::Int(33),
            cell_prepared,
            MAX_STRING_CAPACITY,
        )
    }));
    let value_prepared = StoreAccess::prepare_write(
        &fixture.values,
        MemoryLocation::Global,
        BASE_G,
        &member,
        &Value::Int(33),
    )
    .expect("the member prepares in the value store")
    .expect("the member is a target in the value store");
    assert!(StoreMutate::commit_write(
        &mut fixture.values,
        &member,
        Value::Int(33),
        value_prepared,
        MAX_STRING_CAPACITY
    ));

    // A write through a reference to `dint_g`, with no path.
    let reference = ValueRefView {
        location: MemoryLocation::Global,
        offset: DINT_G,
        generation: Generation::FIRST,
        path: &[],
    };
    assert_eq!(
        with_writer(fixture, |writer| {
            StoreMutate::write_reference(writer, reference, Value::Int(3))
        }),
        Ok(true)
    );
    assert_eq!(
        StoreMutate::write_reference(&mut fixture.values, reference, Value::Int(3)),
        Ok(true)
    );
}

/// Every global and every path read compares the two stores, and the hand values hold.
fn assert_cell_and_value_reads_equal(fixture: &Fixture) {
    // The program root `Main` holds no storage and names no instance the fixture constructs
    // (C1-19e), where the `Value` store held its Null; every other global reads alike.
    assert_eq!(fixture.reader().read_global("Main").map(value_of), None);
    for (ordinal, name) in GLOBAL_NAMES.iter().enumerate().skip(1) {
        let from_cells = fixture.reader().read_global(name).map(value_of);
        assert_eq!(
            from_cells,
            StoreAccess::read_global(&fixture.values, name).map(value_of),
            "global {name} (ordinal {ordinal})"
        );
    }
    assert_eq!(
        fixture.reader().read_global("dint_g").map(value_of),
        Some(Value::DInt(3))
    );
    assert_eq!(
        fixture.reader().read_global("real_g").map(value_of),
        Some(Value::Real(2.5))
    );
    assert_eq!(
        fixture.reader().read_global("text_g").map(value_of),
        Some(Value::String(SmolStr::new("xyz")))
    );

    let paths: [(usize, Vec<RefSegment>, Value); 5] = [
        (BASE_G, vec![field_segment("base_v")], Value::DInt(9)),
        (
            BASE_G,
            vec![field_segment("series_v"), RefSegment::Index(vec![2])],
            Value::Int(7),
        ),
        (
            BASE_G,
            vec![field_segment("pair_v"), field_segment("y")],
            Value::Int(33),
        ),
        (FB_G, vec![field_segment("own_v")], Value::Real(1.5)),
        (FB_G, vec![field_segment("base_v")], Value::DInt(5)),
    ];
    for (ordinal, path, expected) in &paths {
        let from_cells = fixture
            .reader()
            .read_path(MemoryLocation::Global, *ordinal, path)
            .ok()
            .map(value_of);
        assert_eq!(
            from_cells,
            Some(expected.clone()),
            "path of global {ordinal}"
        );
        assert_eq!(
            from_cells,
            StoreAccess::read_path(&fixture.values, MemoryLocation::Global, *ordinal, path)
                .ok()
                .map(value_of),
            "path of global {ordinal} in the value store"
        );
    }
}

#[test]
fn set_global_takes_declared_names_only() {
    // Specification 34 §6.2 (MEM-10, O15): a name no declaration holds is refused and stores nothing;
    // a declared global takes its value. The `Value` store would insert `undeclared` and answer true.
    let mut fixture = Fixture::new();
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::set_global(writer, "undeclared", Value::DInt(1))
    }));
    assert_eq!(
        fixture.reader().read_global("undeclared").map(value_of),
        None
    );
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::set_global(writer, "dint_g", Value::DInt(8))
    }));
    assert_eq!(
        fixture.reader().read_global("dint_g").map(value_of),
        Some(Value::DInt(8))
    );
}

#[test]
fn a_persistent_instance_ends_only_with_its_generation() {
    // Specification 34 §6.2 (the C1-9 note, `remove_instance`): a persistent instance is not released
    // by `remove_instance`; it ends with its generation. The refused removal leaves `base_g` readable, with
    // base_v 5 (the value the fixture wrote into its cells).
    let mut fixture = Fixture::new();
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::remove_instance(writer, BASE_G_INSTANCE)
    }));
    assert_eq!(
        fixture
            .reader()
            .read_instance_var(BASE_G_INSTANCE, "base_v")
            .map(value_of),
        Some(Value::DInt(5))
    );
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::write_instance_var(writer, BASE_G_INSTANCE, "undeclared", Value::DInt(1))
    }));
}

#[test]
fn a_copy_of_a_cell_value_charges_what_the_value_copy_charges() {
    // TIME-08 and C1-9c (R5): the copy charge of a value, computed from its cells, with the units of the
    // `Value` copy worked out by hand. `series_v` (an ARRAY of three INT) costs 1 + 3 = 4 work units, and
    // 3 leaves of one cell (16 bytes) each, 48 bytes. `pair_v` (a Pair of two INT) costs 1 + 2 = 3 units
    // and 2 leaves, 32 bytes. The STRING 'abc' costs its 3 bytes and no work. The WSTRING 'hi' costs one
    // unit and 4 bytes (two UTF-16 code units of two bytes). The instance `base_g` copies nothing.
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let series = reader
        .read_path(MemoryLocation::Global, BASE_G, &[field_segment("series_v")])
        .expect("series_v reads");
    assert_eq!(
        charge_of(series),
        CopyCharge {
            work: 4,
            allocation: 48,
        }
    );
    let pair = reader
        .read_path(MemoryLocation::Global, BASE_G, &[field_segment("pair_v")])
        .expect("pair_v reads");
    assert_eq!(
        charge_of(pair),
        CopyCharge {
            work: 3,
            allocation: 32,
        }
    );
    let text = reader.read_global("text_g").expect("text_g reads");
    assert_eq!(
        charge_of(text),
        CopyCharge {
            work: 0,
            allocation: 3,
        }
    );
    let wide = reader.read_global("wide_g").expect("wide_g reads");
    assert_eq!(
        charge_of(wide),
        CopyCharge {
            work: 1,
            allocation: 4,
        }
    );
    let instance = reader.read_global("base_g").expect("base_g reads");
    assert_eq!(charge_of(instance), CopyCharge::default());
}

#[test]
fn a_character_write_charges_the_utf8_length_of_its_text() {
    // TIME-08 and C1-9c (R5), `charge_string_path_write`: a character write charges the text's length plus
    // four units, three work units and one 16-byte cell of allocation for each unit. `text_g` is 'abc' (3
    // bytes), so 3 + 4 = 7 units: 21 work and 112 bytes. `wide_g` is 'hi', whose UTF-8 length is 2 (not its
    // 4 bytes of UTF-16), so 2 + 4 = 6 units: 18 work and 96 bytes. A path with no index on a text charges
    // nothing.
    let fixture = Fixture::new();
    let reader = fixture.reader();
    assert_eq!(
        reader
            .text_element_charge(
                MemoryLocation::Global,
                TEXT_G,
                &[RefSegment::Index(vec![1])]
            )
            .expect("a charge"),
        Some(CopyCharge {
            work: 21,
            allocation: 112,
        })
    );
    assert_eq!(
        reader
            .text_element_charge(
                MemoryLocation::Global,
                WIDE_G,
                &[RefSegment::Index(vec![1])]
            )
            .expect("a charge"),
        Some(CopyCharge {
            work: 18,
            allocation: 96,
        })
    );
    assert_eq!(
        reader
            .text_element_charge(MemoryLocation::Global, BASE_G, &[field_segment("series_v")])
            .expect("no charge is not a refusal"),
        None
    );
    // The declared name of the global at ordinal FB_G (6) is `fb_g`, in declaration order.
    assert_eq!(reader.global_name(FB_G), Some("fb_g"));
}

#[test]
fn a_reference_leaf_holds_its_value_ref_on_the_value_side() {
    // Specification 34 §6.2 and O9 (R3, C1-9c): a non-null reference leaf holds its `ValueRef` on the
    // `Value` side and reads back as the reference written. `ref_g` is a `REF_TO DINT` of one cell. The
    // reference written names `base_g.pair_v.y` (`base_g` is ordinal 5), a path of two field segments, so
    // its copy costs 1 + 2 = 3 work units and 2 cells of allocation (32 bytes). A null reference frees the
    // leaf, and an INT is refused, which leaves the leaf as it was.
    let mut fixture = Fixture::new();
    let target = ValueRef {
        location: MemoryLocation::Global,
        offset: 5,
        generation: Generation::FIRST,
        path: vec![field_segment("pair_v"), field_segment("y")],
    };
    let reference = Value::Reference(Some(target));
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, REF_G, reference.clone())
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        REF_G,
        reference.clone()
    ));
    assert_eq!(
        fixture.reader().read_global("ref_g").map(value_of),
        Some(reference.clone())
    );
    assert_eq!(
        StoreAccess::read_global(&fixture.values, "ref_g").map(value_of),
        Some(reference)
    );
    let view = fixture.reader().read_global("ref_g").expect("ref_g reads");
    assert_eq!(
        charge_of(view),
        CopyCharge {
            work: 3,
            allocation: 32,
        }
    );
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(
            writer,
            MemoryLocation::Global,
            REF_G,
            Value::Reference(None),
        )
    }));
    assert_eq!(
        fixture.reader().read_global("ref_g").map(value_of),
        Some(Value::Reference(None))
    );
    assert!(!with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, REF_G, Value::Int(3))
    }));
    assert_eq!(
        fixture.reader().read_global("ref_g").map(value_of),
        Some(Value::Reference(None))
    );
}

#[test]
fn a_snapshot_restores_the_members_it_holds() {
    // Specification 34 MEM-09 (the initializer rollback, `restore_instance`): a snapshot put back over the
    // record restores the members it holds. The snapshot of `base_g` holds base_v 5. After base_v is
    // written 9, restoring the snapshot reads base_v 5 again.
    let mut fixture = Fixture::new();
    let snapshot = fixture
        .reader()
        .snapshot_instance(BASE_G_INSTANCE)
        .expect("base_g has a record");
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_instance_var(writer, BASE_G_INSTANCE, "base_v", Value::DInt(9))
    }));
    assert_eq!(
        fixture
            .reader()
            .read_instance_var(BASE_G_INSTANCE, "base_v")
            .map(value_of),
        Some(Value::DInt(9))
    );
    with_writer(&mut fixture, |writer| {
        StoreMutate::restore_instance(writer, BASE_G_INSTANCE, snapshot);
    });
    assert_eq!(
        fixture
            .reader()
            .read_instance_var(BASE_G_INSTANCE, "base_v")
            .map(value_of),
        Some(Value::DInt(5))
    );
}

#[test]
fn a_saved_slot_is_restored_as_it_was_saved() {
    // Specification 34 MEM-09 (`restore_slot`, the journals): a saved root goes back without normalization.
    // `dint_g` is saved as DINT 7, written 9, and restored to 7.
    let mut fixture = Fixture::new();
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, DINT_G, Value::DInt(9))
    }));
    with_writer(&mut fixture, |writer| {
        StoreMutate::restore_slot(writer, MemoryLocation::Global, DINT_G, Value::DInt(7));
    });
    assert_eq!(
        fixture.reader().read_global("dint_g").map(value_of),
        Some(Value::DInt(7))
    );
}

/// The `Holder` value of a reference `r` and a DINT `n`: the structure `holder_g` is declared as.
fn holder_value(reference: Option<ValueRef>, n: i32) -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("r"), Value::Reference(reference));
    fields.insert(SmolStr::new("n"), Value::DInt(n));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Holder"),
        fields,
    )))
}

/// The fixture of the `holder_g` module: its cell arm holds the holder's defaults, written at construction
/// (R2). Its `Value` half is empty, since these tests read the cells only.
fn holder_fixture() -> Fixture {
    let prepared = prepared_with_holder();
    let cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
        .expect("the holder's region reserves");
    Fixture {
        prepared,
        arm: CellArm::new(cells),
        half: VariableStorage::new(),
        values: VariableStorage::new(),
    }
}

#[test]
fn a_reference_field_in_a_global_structure_round_trips() {
    // Specification 34 §6.2, O9 and R3: a reference leaf nested in a structure is written, read and
    // materialized through the cells. `holder_g` is a `Holder`, two cells: `r`, a `REF_TO DINT` whose cell
    // names its entry in the reference table, and `n`, a DINT. Worked out by hand: before any write `r` is
    // null and `n` is 0 (the defaults construction writes). Written: `r` names `base_g.pair_v.y` (ordinal 5,
    // path pair_v.y) and `n` is 4. The structure reads back as written, and `r` reads back by its path. A
    // null `r` written through its path leaves `n` at 4. An INT written to `r` is refused and changes
    // nothing. A reader that dropped the nested reference would read `r` as a null or refuse the struct.
    let mut fixture = holder_fixture();
    let target = ValueRef {
        location: MemoryLocation::Global,
        offset: 5,
        generation: Generation::FIRST,
        path: vec![field_segment("pair_v"), field_segment("y")],
    };
    assert_eq!(
        fixture.reader().read_global("holder_g").map(value_of),
        Some(holder_value(None, 0))
    );

    let written = holder_value(Some(target.clone()), 4);
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, HOLDER_G, written.clone())
    }));
    assert_eq!(
        fixture.reader().read_global("holder_g").map(value_of),
        Some(written)
    );
    let r_path = [field_segment("r")];
    assert_eq!(
        fixture
            .reader()
            .read_path(MemoryLocation::Global, HOLDER_G, &r_path)
            .ok()
            .map(value_of),
        Some(Value::Reference(Some(target)))
    );

    let r_null = ValueRefView {
        location: MemoryLocation::Global,
        offset: HOLDER_G,
        generation: Generation::FIRST,
        path: &r_path,
    };
    assert_eq!(
        with_writer(&mut fixture, |writer| {
            StoreMutate::write_reference(writer, r_null, Value::Reference(None))
        }),
        Ok(true)
    );
    assert_eq!(
        fixture.reader().read_global("holder_g").map(value_of),
        Some(holder_value(None, 4))
    );

    let r_int = ValueRefView {
        location: MemoryLocation::Global,
        offset: HOLDER_G,
        generation: Generation::FIRST,
        path: &r_path,
    };
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_reference(writer, r_int, Value::Int(3))
    })
    .is_err());
    assert_eq!(
        fixture.reader().read_global("holder_g").map(value_of),
        Some(holder_value(None, 4))
    );
}

#[test]
fn a_reference_write_in_a_scan_allocates_nothing() {
    // Specification 34 §6.2 (O9, R5): a reference write takes no new reservation. The reference table is
    // reserved with the reference leaves at construction, so its capacity is the same after the write and
    // the write adds no entry. Its staged write holds no copy of the reference: the check reads it by
    // reference and the commit moves the value it is given, so no path is cloned. `ref_g` takes the
    // reference to `base_g.pair_v.y`. A reference write that cloned its path would leave a `CellPatch`
    // holding the value, and a table that grew would change its capacity.
    //
    // This is the structural proof. The counting-allocator harness of `tests/scan_allocation_zero.rs`
    // cannot reach the cell arm, which the engine does not build until C1-9d.
    let mut fixture = Fixture::new();
    let capacity = fixture.arm.cells.regions().references().capacity();
    let reference = Value::Reference(Some(ValueRef {
        location: MemoryLocation::Global,
        offset: 5,
        generation: Generation::FIRST,
        path: vec![field_segment("pair_v"), field_segment("y")],
    }));
    let staged = fixture
        .reader()
        .prepare_write(MemoryLocation::Global, REF_G, &[], &reference)
        .expect("the reference stages")
        .expect("ref_g is a write target");
    assert!(matches!(
        staged.0,
        PreparedWriteKind::Cells(CellPatch { leaf: None, .. })
    ));
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::commit_write(writer, &[], reference.clone(), staged, MAX_STRING_CAPACITY)
    }));
    assert_eq!(
        fixture.reader().read_global("ref_g").map(value_of),
        Some(reference)
    );
    assert_eq!(
        fixture.arm.cells.regions().references().capacity(),
        capacity
    );
}

/// What a view of a text answers about its characters: their count, and the character at each of the
/// indices 0 to 3.
type Characters = (
    Result<Option<usize>, RuntimeError>,
    Vec<Result<Option<Value>, RuntimeError>>,
);

fn characters(view: ValueView<'_>) -> Characters {
    (
        view.text_char_count(),
        [0i64, 1, 2, 3]
            .map(|index| view.text_element(&[index]))
            .to_vec(),
    )
}

#[test]
fn a_cell_wstring_counts_and_reads_its_characters_from_its_utf16_code_units() {
    // Specification 34 §6.2 and DEV-017: `LEN` and a subscript count characters, the first of which is
    // index 1, and a `WCHAR` holds one UTF-16 code unit. `wide_g : WSTRING[5]` is 'hi': 2 characters,
    // 'h' (104) at index 1 and 'i' (105) at index 2, and indices 0 and 3 name none. `text_g :
    // STRING[10]` is 'abc': 3 characters. `dint_g` holds no text. After 'a', U+1F642 and 'b' are written
    // to `wide_g` (the middle character is two code units, so the four code units fit the capacity of
    // five), the text has 3 characters: 'a' (97) at index 1, 'b' (98) at index 3, and none at index 2,
    // since that character does not fit a `WCHAR`, as the `Value` store answers. A cell store that
    // counted code units (4) or UTF-8 bytes (6) would count wrongly, and one that refused the read of a
    // WSTRING character would answer an error instead of the character.
    let mut fixture = Fixture::new();
    let hi = (
        Ok(Some(2)),
        vec![
            Ok(None),
            Ok(Some(Value::WChar(104))),
            Ok(Some(Value::WChar(105))),
            Ok(None),
        ],
    );
    let read = |fixture: &Fixture, name: &str| {
        (
            characters(
                fixture
                    .reader()
                    .read_global(name)
                    .expect("the global reads"),
            ),
            characters(StoreAccess::read_global(&fixture.values, name).expect("the global reads")),
        )
    };
    let (cells, values) = read(&fixture, "wide_g");
    assert_eq!(cells, hi, "cells");
    assert_eq!(values, hi, "values");
    assert_eq!(read(&fixture, "text_g").0 .0, Ok(Some(3)));
    assert_eq!(read(&fixture, "dint_g").0 .0, Ok(None));

    let mixed = Value::WString(String::from("a\u{1F642}b"));
    assert!(with_writer(&mut fixture, |writer| {
        StoreMutate::write_slot(writer, MemoryLocation::Global, WIDE_G, mixed.clone())
    }));
    assert!(StoreMutate::write_slot(
        &mut fixture.values,
        MemoryLocation::Global,
        WIDE_G,
        mixed
    ));
    let expected = (
        Ok(Some(3)),
        vec![
            Ok(None),
            Ok(Some(Value::WChar(97))),
            Ok(None),
            Ok(Some(Value::WChar(98))),
        ],
    );
    let (cells, values) = read(&fixture, "wide_g");
    assert_eq!(cells, expected, "cells");
    assert_eq!(values, expected, "values");
}
