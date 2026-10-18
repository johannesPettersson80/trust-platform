//! Tests of the cell arm's reads (specification 34 §6.2, MEM-10; C1-9b). They sit in the crate
//! because they fill the cells through the test seams of `memory::regions`; the write side that
//! fills them in the engine is C1-9c.
//!
//! The ST source the bytecode fixture (`super::fixture`) encodes, from which every expected value is
//! worked out by hand:
//!
//! ```text
//! TYPE Pair : STRUCT x : INT; y : INT; END_STRUCT END_TYPE
//! FUNCTION_BLOCK Base
//!     VAR base_v : DINT; series_v : ARRAY[1..3] OF INT; pair_v : Pair; END_VAR
//! END_FUNCTION_BLOCK
//! FUNCTION_BLOCK Derived EXTENDS Base VAR own_v : REAL; END_VAR END_FUNCTION_BLOCK
//! VAR_GLOBAL
//!     dint_g : DINT;  real_g : REAL;  text_g : STRING[10];  wide_g : WSTRING[5];
//!     base_g : Base;  fb_g : Derived;
//! END_VAR
//! ```
//!
//! `base_g` and `fb_g` are persistent roots. The `Base` root holds 6 cells: `base_v` (1), `series_v`
//! (3) and `pair_v` (2). `fb_g` is a `Derived` with one cell, `own_v`, whose parent is the instance
//! `base_g` holds (the parent link the tests record by hand, until construction does).

use std::collections::BTreeMap;

use smol_str::SmolStr;

use super::fixture::{
    pair_value, prepared, series_value, BASE, BASE_DECL, BASE_G, BASE_INSTANCE, BASE_POU,
    BASE_ROOT, DERIVED, DERIVED_INSTANCE, DERIVED_POU, DINT, DINT_DECL, DINT_G, FB_DECL, FB_G,
    FB_ROOT, GLOBAL_NAMES, INT_ARRAY, PAIR, REAL, REAL_DECL, STRING_10, TEXT_DECL, WIDE_DECL,
    WSTRING_5,
};
use super::{CellArm, CellReader};
use crate::memory::layout::CellRange;
use crate::memory::regions::CellStorage;
use crate::memory::view::{StoreAccess, ValueView};
use crate::memory::{Generation, MemoryLocation, VariableStorage};
use crate::value::{RefSegment, Value, ValueRefView};
use crate::vm::{PreparedModule, VmTrap};

/// Write `value` of `type_id` into the cells of `range`, through the same `write_value` the engine's
/// write side will use.
fn write(
    cells: &mut CellStorage,
    prepared: &PreparedModule,
    range: CellRange,
    type_id: u32,
    value: &Value,
) {
    let (persistent, text) = cells.regions_mut().parts_mut();
    let slice = persistent
        .cells_mut(range)
        .expect("the range lies in the persistent region");
    crate::memory::cells::write_value(slice, type_id, value, prepared, text, range.start)
        .expect("the fixture value writes");
}

/// The cell range of the global declaration `declaration`.
fn global(cells: &CellStorage, declaration: u32) -> CellRange {
    cells
        .regions()
        .global_range(declaration as usize)
        .expect("the global owns cells")
}

/// The fixture's initial values, written into the persistent region (specification 34 §6.2).
fn write_fixture_cells(cells: &mut CellStorage, prepared: &PreparedModule) {
    let base = cells
        .regions()
        .root_range(BASE_ROOT)
        .expect("the Base root has cells");
    let derived = cells
        .regions()
        .root_range(FB_ROOT)
        .expect("the Derived root has cells");
    let writes = [
        (global(cells, DINT_DECL), DINT, Value::DInt(7)),
        (global(cells, REAL_DECL), REAL, Value::Real(1.5)),
        (
            global(cells, TEXT_DECL),
            STRING_10,
            Value::String(SmolStr::new("abc")),
        ),
        (
            global(cells, WIDE_DECL),
            WSTRING_5,
            Value::WString(String::from("hi")),
        ),
        (
            global(cells, BASE_DECL),
            BASE,
            Value::Instance(BASE_INSTANCE),
        ),
        (
            global(cells, FB_DECL),
            DERIVED,
            Value::Instance(DERIVED_INSTANCE),
        ),
        (
            CellRange {
                region: base.region,
                start: base.start,
                cells: 1,
            },
            DINT,
            Value::DInt(5),
        ),
        (
            CellRange {
                region: base.region,
                start: base.start + 1,
                cells: 3,
            },
            INT_ARRAY,
            series_value(),
        ),
        (
            CellRange {
                region: base.region,
                start: base.start + 4,
                cells: 2,
            },
            PAIR,
            pair_value(),
        ),
        (derived, REAL, Value::Real(2.5)),
    ];
    for (range, type_id, value) in &writes {
        write(cells, prepared, *range, *type_id, value);
    }
}

/// The `Value` store holding the same globals and instances as the cell arm.
fn values_store() -> VariableStorage {
    let mut values = VariableStorage::new();
    values.set_global("Main", Value::Null);
    values.set_global("dint_g", Value::DInt(7));
    values.set_global("real_g", Value::Real(1.5));
    values.set_global("text_g", Value::String(SmolStr::new("abc")));
    values.set_global("wide_g", Value::WString(String::from("hi")));
    let base = values.try_create_instance("Base").expect("a Base instance");
    let derived = values
        .try_create_instance("Derived")
        .expect("a Derived instance");
    assert_eq!((base, derived), (BASE_INSTANCE, DERIVED_INSTANCE));
    values.set_instance_var(base, "base_v", Value::DInt(5));
    values.set_instance_var(base, "series_v", series_value());
    values.set_instance_var(base, "pair_v", pair_value());
    values.set_instance_var(derived, "own_v", Value::Real(2.5));
    if let Some(record) = values.get_instance_mut(derived) {
        record.parent = Some(base);
    }
    values.set_global("base_g", Value::Instance(base));
    values.set_global("fb_g", Value::Instance(derived));
    values
}

struct Fixture {
    prepared: PreparedModule,
    arm: CellArm,
    values: VariableStorage,
}

impl Fixture {
    fn new() -> Self {
        let prepared = prepared();
        let mut cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
            .expect("the persistent region reserves");
        write_fixture_cells(&mut cells, &prepared);
        let base_range = cells
            .regions()
            .root_range(BASE_ROOT)
            .expect("the Base root has cells");
        let derived_range = cells
            .regions()
            .root_range(FB_ROOT)
            .expect("the Derived root has cells");
        let mut arm = CellArm {
            cells,
            values: VariableStorage::new(),
            instances: BTreeMap::new(),
        };
        arm.register_instance(BASE_INSTANCE, BASE_POU, base_range, None, "Base");
        arm.register_instance(
            DERIVED_INSTANCE,
            DERIVED_POU,
            derived_range,
            Some(BASE_INSTANCE),
            "Derived",
        );
        Self {
            prepared,
            arm,
            values: values_store(),
        }
    }

    fn reader(&self) -> CellReader<'_> {
        CellReader::new(&self.prepared, &self.arm)
    }
}

/// The owned value a view names, read through the view's own cells or borrowed `Value`.
fn value_of(view: ValueView<'_>) -> Value {
    view.materialize().expect("the view materializes")
}

fn field_segment(name: &str) -> RefSegment {
    RefSegment::Field(SmolStr::new(name))
}

#[test]
fn read_of_a_foreign_generation_reference_is_null() {
    // Specification 34 §11.3 and MEM-07: a reference of another installation generation names no
    // state in this store, so both arms refuse it with NullReference. The same slot referenced in
    // this store's generation reads 7. A reader that skipped the generation check would read 7 from
    // the foreign reference.
    let fixture = Fixture::new();
    let foreign = ValueRefView {
        location: MemoryLocation::Global,
        offset: DINT_G,
        generation: Generation(1),
        path: &[],
    };
    let own = ValueRefView {
        generation: Generation::FIRST,
        ..foreign
    };
    assert!(matches!(
        StoreAccess::read_reference(&fixture.values, foreign),
        Err(VmTrap::NullReference)
    ));
    assert!(matches!(
        fixture.reader().read_reference(foreign),
        Err(VmTrap::NullReference)
    ));
    assert_eq!(
        StoreAccess::read_reference(&fixture.values, own)
            .ok()
            .map(value_of),
        Some(Value::DInt(7))
    );
    assert_eq!(
        fixture.reader().read_reference(own).ok().map(value_of),
        Some(Value::DInt(7))
    );
}

#[test]
fn read_of_a_missing_member_is_null_reference() {
    // Specification 34 §6.2 and the member rule of `read_path`: a path that names no member of its
    // structure is a null reference, never a default value. Both arms refuse `base_g.pair_v.z`;
    // `base_g.pair_v.y` reads 20. A reader that returned a default for the missing member would not
    // refuse it.
    let fixture = Fixture::new();
    let missing = [field_segment("pair_v"), field_segment("z")];
    let present = [field_segment("pair_v"), field_segment("y")];
    assert!(matches!(
        StoreAccess::read_path(&fixture.values, MemoryLocation::Global, BASE_G, &missing),
        Err(VmTrap::NullReference)
    ));
    assert!(matches!(
        fixture
            .reader()
            .read_path(MemoryLocation::Global, BASE_G, &missing),
        Err(VmTrap::NullReference)
    ));
    assert_eq!(
        fixture
            .reader()
            .read_path(MemoryLocation::Global, BASE_G, &present)
            .ok()
            .map(value_of),
        Some(Value::Int(20))
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one read of every method, each compared with the Value store"
)]
fn cell_reads_equal_value_reads() {
    // Specification 34 §6.2 (MEM-10) and §3.1 of the C1-9 note: every read of the cell arm gives what
    // the `Value` store gives for the same program. The expected values are worked out from the ST
    // source above: dint_g = 7, text_g = 'abc', base_g.series_v[2] = 2, base_g.pair_v.y = 20,
    // base_v = 5 (on Base and, through the parent link, on Derived) and own_v = 2.5.
    let fixture = Fixture::new();
    let reader = fixture.reader();
    let values = &fixture.values;

    for (ordinal, name) in GLOBAL_NAMES.iter().enumerate() {
        assert_eq!(
            reader.global_name(ordinal),
            StoreAccess::global_name(values, ordinal),
            "the name of global ordinal {ordinal}"
        );
        assert_eq!(
            reader
                .ref_for_global(name)
                .map(|reference| reference.offset),
            StoreAccess::ref_for_global(values, name).map(|reference| reference.offset),
            "the reference to {name}"
        );
        // The program's root is the first global. It holds no cells, so the variables are read.
        if ordinal == 0 {
            continue;
        }
        let from_values = StoreAccess::read_global(values, name).map(value_of);
        assert_eq!(
            reader.read_global(name).map(value_of),
            from_values,
            "global {name}"
        );
        assert_eq!(
            reader
                .read_slot(MemoryLocation::Global, ordinal)
                .ok()
                .map(value_of),
            StoreAccess::read_slot(values, MemoryLocation::Global, ordinal)
                .ok()
                .map(value_of),
            "the slot of {name}"
        );
    }
    assert_eq!(
        reader.read_global("dint_g").map(value_of),
        Some(Value::DInt(7))
    );
    assert_eq!(
        reader.read_global("text_g").map(value_of),
        Some(Value::String(SmolStr::new("abc")))
    );

    let paths: [(usize, Vec<RefSegment>, Value); 5] = [
        (
            BASE_G,
            vec![field_segment("pair_v"), field_segment("y")],
            Value::Int(20),
        ),
        (
            BASE_G,
            vec![field_segment("series_v"), RefSegment::Index(vec![2])],
            Value::Int(2),
        ),
        (FB_G, vec![field_segment("own_v")], Value::Real(2.5)),
        (FB_G, vec![field_segment("base_v")], Value::DInt(5)),
        (BASE_G, vec![field_segment("base_v")], Value::DInt(5)),
    ];
    for (ordinal, path, expected) in &paths {
        let from_values = StoreAccess::read_path(values, MemoryLocation::Global, *ordinal, path)
            .ok()
            .map(value_of);
        let from_cells = reader
            .read_path(MemoryLocation::Global, *ordinal, path)
            .ok()
            .map(value_of);
        assert_eq!(
            from_cells,
            Some(expected.clone()),
            "path of global {ordinal}"
        );
        assert_eq!(from_cells, from_values, "path of global {ordinal}");
    }

    assert_eq!(
        reader
            .read_instance_var(BASE_INSTANCE, "base_v")
            .map(value_of),
        StoreAccess::read_instance_var(values, BASE_INSTANCE, "base_v").map(value_of),
        "base_v of Base"
    );
    assert_eq!(
        reader
            .read_instance_var(DERIVED_INSTANCE, "base_v")
            .map(value_of),
        None,
        "a Derived instance declares no base_v of its own"
    );
    assert!(StoreAccess::read_instance_var(values, DERIVED_INSTANCE, "base_v").is_none());
    for (name, expected) in [("base_v", Value::DInt(5)), ("own_v", Value::Real(2.5))] {
        let from_cells = reader
            .read_instance_field(DERIVED_INSTANCE, name)
            .ok()
            .map(value_of);
        assert_eq!(from_cells, Some(expected), "field {name} of Derived");
        assert_eq!(
            from_cells,
            StoreAccess::read_instance_field(values, DERIVED_INSTANCE, name)
                .ok()
                .map(value_of),
            "field {name} of Derived"
        );
    }

    assert_eq!(
        reader.ref_for_instance(DERIVED_INSTANCE, "own_v"),
        StoreAccess::ref_for_instance(values, DERIVED_INSTANCE, "own_v"),
        "the reference to own_v"
    );
    let inherited = reader
        .ref_for_instance_recursive(DERIVED_INSTANCE, "base_v")
        .expect("base_v is reachable from Derived");
    assert_eq!(
        (inherited.location, inherited.offset),
        (MemoryLocation::Instance(BASE_INSTANCE), 0)
    );
    assert_eq!(
        reader.resolved_instance_field_ref(DERIVED_INSTANCE, "base_v"),
        StoreAccess::resolved_instance_field_ref(values, DERIVED_INSTANCE, "base_v"),
        "the resolved reference to base_v"
    );
    assert_eq!(
        reader.declared_instance_field_offset(DERIVED_INSTANCE, "own_v"),
        Some(0)
    );
    assert_eq!(
        reader.declared_instance_field_offset(DERIVED_INSTANCE, "base_v"),
        None
    );
    assert_eq!(
        reader
            .instance_type_name(DERIVED_INSTANCE)
            .map(SmolStr::as_str),
        Some("Derived")
    );
    assert_eq!(
        reader.instance_type_name(DERIVED_INSTANCE),
        StoreAccess::instance_type_name(values, DERIVED_INSTANCE)
    );
    assert_eq!(
        reader.instance_parent(DERIVED_INSTANCE).ok(),
        Some(Some(BASE_INSTANCE))
    );
    assert_eq!(
        StoreAccess::instance_parent(values, DERIVED_INSTANCE).ok(),
        Some(Some(BASE_INSTANCE))
    );

    let record = reader
        .snapshot_instance(DERIVED_INSTANCE)
        .expect("the Derived record");
    let expected = StoreAccess::snapshot_instance(values, DERIVED_INSTANCE)
        .expect("the Derived record in the Value store");
    assert_eq!(record.type_name, expected.type_name);
    assert_eq!(record.parent, expected.parent);
    assert_eq!(
        record.variables.iter().collect::<Vec<_>>(),
        expected.variables.iter().collect::<Vec<_>>()
    );
    assert_eq!(reader.generation(), StoreAccess::generation(values));
}

#[test]
fn wstring_text_is_none_on_the_cells() {
    // Specification 34 §3.1 of the C1-9 note: the text of a cell view is the UTF-8 of a STRING and
    // None for a WSTRING, whose buffer holds UTF-16 code units. A reader that gave the text of
    // `wide_g` would answer "hi" here. The view still materializes the WSTRING value, and a cell view
    // borrows no `Value`.
    let fixture = Fixture::new();
    let wide = fixture
        .reader()
        .read_global("wide_g")
        .expect("wide_g reads");
    assert_eq!(wide.text(), None);
    assert_eq!(wide.borrowed_value(), None);
    assert_eq!(value_of(wide), Value::WString(String::from("hi")));
    let text = fixture
        .reader()
        .read_global("text_g")
        .expect("text_g reads");
    assert_eq!(text.text(), Some("abc"));
}
