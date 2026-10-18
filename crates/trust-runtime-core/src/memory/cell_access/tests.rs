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
//! (3) and `pair_v` (2). `fb_g` is a `Derived` whose root covers its whole chain (R0, C1-9c): its own
//! cell `own_v` (1), then the 6 cells of its parent's template, where the parent instance construction
//! creates for `fb_g` lives. The parent is a separate instance from the one `base_g` holds.

use std::sync::Arc;

use smol_str::SmolStr;

use super::fixture::{
    pair_value, prepared, prepared_without_parent_root, series_value, BASE, BASE_DECL, BASE_G,
    BASE_G_INSTANCE, BASE_INSTANCE, BASE_POU, BASE_ROOT, DERIVED, DERIVED_INSTANCE, DERIVED_POU,
    DINT, DINT_DECL, DINT_G, FB_DECL, FB_G, FB_ROOT, GLOBAL_NAMES, INT_ARRAY, PAIR, PARENT_ROOT,
    REAL, REAL_DECL, REF_DECL, REF_DINT, STRING_10, TEXT_DECL, WIDE_DECL, WSTRING_5,
};
use super::{CellArm, CellReader, CellWriter};
use crate::collections::OrderedMap;
use crate::error::RuntimeError;
use crate::memory::layout::{CellRange, LayoutRegion};
use crate::memory::regions::CellStorage;
use crate::memory::view::{StoreAccess, StoreMutate, ValueView};
use crate::memory::{Generation, InstanceId, MemoryLocation, VariableStorage};
use crate::value::{ArrayValue, RefSegment, StructValue, Value, ValueRefView};
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

/// The `Base` members of the instance whose cells start at `base`: `base_v` (1 cell), `series_v` (3)
/// and `pair_v` (2), at the offsets the `Base` layout gives them.
fn write_base_members(cells: &mut CellStorage, prepared: &PreparedModule, base: CellRange) {
    let writes = [
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
    ];
    for (range, type_id, value) in &writes {
        write(cells, prepared, *range, *type_id, value);
    }
}

/// The fixture's initial values, written into the persistent region (specification 34 §6.2).
fn write_fixture_cells(cells: &mut CellStorage, prepared: &PreparedModule) {
    let base = cells
        .regions()
        .root_range(BASE_ROOT)
        .expect("the Base root has cells");
    let parent = cells
        .regions()
        .root_range(PARENT_ROOT)
        .expect("the parent root has cells");
    let derived = cells
        .regions()
        .root_range(FB_ROOT)
        .expect("the Derived root has cells");
    let derived_own = CellRange {
        region: derived.region,
        start: derived.start,
        cells: 1,
    };
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
            Value::Instance(BASE_G_INSTANCE),
        ),
        (
            global(cells, FB_DECL),
            DERIVED,
            Value::Instance(DERIVED_INSTANCE),
        ),
        (derived_own, REAL, Value::Real(2.5)),
        (global(cells, REF_DECL), REF_DINT, Value::Reference(None)),
    ];
    for (range, type_id, value) in &writes {
        write(cells, prepared, *range, *type_id, value);
    }
    write_base_members(cells, prepared, base);
    write_base_members(cells, prepared, parent);
}

/// The `Value` store holding the same globals and instances as the cell arm: the inheritance parent
/// of `fb_g`'s instance, `fb_g`'s instance and the instance `base_g` holds, in that order.
pub(super) fn values_store() -> VariableStorage {
    let mut values = VariableStorage::new();
    values.set_global("Main", Value::Null);
    values.set_global("dint_g", Value::DInt(7));
    values.set_global("real_g", Value::Real(1.5));
    values.set_global("text_g", Value::String(SmolStr::new("abc")));
    values.set_global("wide_g", Value::WString(String::from("hi")));
    let parent = values.try_create_instance("Base").expect("a Base instance");
    let derived = values
        .try_create_instance("Derived")
        .expect("a Derived instance");
    let base = values.try_create_instance("Base").expect("a Base instance");
    assert_eq!(
        (parent, derived, base),
        (BASE_INSTANCE, DERIVED_INSTANCE, BASE_G_INSTANCE)
    );
    for instance in [parent, base] {
        values.set_instance_var(instance, "base_v", Value::DInt(5));
        values.set_instance_var(instance, "series_v", series_value());
        values.set_instance_var(instance, "pair_v", pair_value());
    }
    values.set_instance_var(derived, "own_v", Value::Real(2.5));
    if let Some(record) = values.get_instance_mut(derived) {
        record.parent = Some(parent);
    }
    values.set_global("base_g", Value::Instance(base));
    values.set_global("fb_g", Value::Instance(derived));
    values.set_global("ref_g", Value::Reference(None));
    values
}

/// The fixture: the cell arm, the `Value` store that holds the same program, and the prepared module
/// that places both.
pub(super) struct Fixture {
    pub(super) prepared: PreparedModule,
    pub(super) arm: CellArm,
    /// The `Value` half the engine lends the arm: it holds the program root, which owns no cells.
    pub(super) half: VariableStorage,
    pub(super) values: VariableStorage,
}

impl Fixture {
    /// The fixture, with the same initial values in both stores. The arm's `Value` half holds the
    /// program root, which owns no cells, and the instance records of construction (R0).
    pub(super) fn new() -> Self {
        Self::build(true)
    }

    /// The fixture with only what construction writes (R2): the defaults of the declared leaves, and
    /// no fixture value written over them.
    pub(super) fn constructed() -> Self {
        Self::build(false)
    }

    fn build(write_initial_values: bool) -> Self {
        let prepared = prepared();
        let mut cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
            .expect("the persistent region reserves");
        // The persistent instances take their identities from the store's one counter, in the order
        // the fixture registers them (R4).
        for expected in [BASE_INSTANCE, DERIVED_INSTANCE, BASE_G_INSTANCE] {
            assert_eq!(cells.issue_instance_id().ok(), Some(expected));
        }
        if write_initial_values {
            write_fixture_cells(&mut cells, &prepared);
        }
        let base_range = cells
            .regions()
            .root_range(BASE_ROOT)
            .expect("the Base root has cells");
        let parent_range = cells
            .regions()
            .root_range(PARENT_ROOT)
            .expect("the parent root has cells");
        let derived_root = cells
            .regions()
            .root_range(FB_ROOT)
            .expect("the Derived root has cells");
        // The derived instance's own cell comes first in its root's range; its parent follows.
        let derived_range = CellRange {
            region: derived_root.region,
            start: derived_root.start,
            cells: 1,
        };
        let mut arm = CellArm::new(cells);
        let mut half = VariableStorage::new();
        half.set_global("Main", Value::Null);
        // The instance table reserves a place for each of the fixture's four instance roots (the program,
        // `base_g`, `fb_g` and the parent of `fb_g`'s instance), so these three fit. The type name of
        // each is its template POU's name, `Base`, `Derived` and `Base`.
        arm.register_instance(BASE_INSTANCE, BASE_POU, parent_range, None)
            .expect("the instance table has room for the parent instance");
        arm.register_instance(
            DERIVED_INSTANCE,
            DERIVED_POU,
            derived_range,
            Some(BASE_INSTANCE),
        )
        .expect("the instance table has room for the derived instance");
        arm.register_instance(BASE_G_INSTANCE, BASE_POU, base_range, None)
            .expect("the instance table has room for the base instance");
        Self {
            prepared,
            arm,
            half,
            values: values_store(),
        }
    }

    pub(super) fn reader(&self) -> CellReader<'_> {
        CellReader::new(&self.prepared, &self.arm, &self.half)
    }
}

/// The owned value a view names, read through the view's own cells or borrowed `Value`.
pub(super) fn value_of(view: ValueView<'_>) -> Value {
    view.materialize().expect("the view materializes")
}

pub(super) fn field_segment(name: &str) -> RefSegment {
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

#[test]
fn a_derived_root_reserves_its_parents_cells() {
    // Specification 34 §6.2 (MEM-10) and O6, the C1-9b finding (R0): a root's range covers its whole
    // inheritance chain. The seven globals before the roots take cells 0 to 6 (`dint_g`, `real_g`,
    // `text_g`, `wide_g`, `base_g`, `fb_g` and `ref_g`, one cell each: a function-block global holds its
    // instance in one cell, a reference global holds its reference in one cell). `base_g`'s root takes
    // cells 7 to 12, its six `Base` cells. `fb_g`'s root takes cells 13 to 19: its own `own_v` (one REAL
    // cell) at 13, then the six cells of its parent's template (`base_v` 1, `series_v` 3, `pair_v` 2) at
    // 14 to 19, where the parent instance lies. The parent has no root of its own. A root that reserved
    // only its own template would give `fb_g` one cell and leave the parent with none, so this test
    // fails then.
    let fixture = Fixture::new();
    let regions = fixture.arm.cells.regions();
    assert_eq!(
        regions.root_range(BASE_ROOT),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 7,
            cells: 6,
        })
    );
    assert_eq!(
        regions.root_range(FB_ROOT),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 13,
            cells: 7,
        })
    );
    assert_eq!(
        regions.root_range(PARENT_ROOT),
        Some(CellRange {
            region: LayoutRegion::Persistent,
            start: 14,
            cells: 6,
        })
    );
    // The parent's `base_v` is the cell at 14, which the fixture wrote as 5 (specification 34 §6.2).
    assert_eq!(
        fixture
            .reader()
            .read_instance_var(BASE_INSTANCE, "base_v")
            .map(value_of),
        Some(Value::DInt(5))
    );
    assert_eq!(
        fixture
            .reader()
            .read_instance_field(DERIVED_INSTANCE, "base_v")
            .ok()
            .map(value_of),
        Some(Value::DInt(5))
    );
}

/// The `ARRAY[1..3] OF INT` of zeros, the default of `series_v`.
fn zero_series() -> Value {
    Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::Int(0); 3],
        vec![(1, 3)],
    )))
}

/// The `Pair` of zeros, the default of `pair_v`.
fn zero_pair() -> Value {
    let mut fields = OrderedMap::default();
    fields.insert(SmolStr::new("x"), Value::Int(0));
    fields.insert(SmolStr::new("y"), Value::Int(0));
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        SmolStr::new("Pair"),
        fields,
    )))
}

#[test]
fn a_constructed_store_reads_every_declared_initial_value() {
    // Specification 34 §6.2 (R2): construction writes each declared leaf's initial value into its cells,
    // the default of its type (the fixture declares no constants). Worked out by hand: DINT 0, REAL 0.0,
    // STRING '' and WSTRING '', the reference null, `series_v` 0, 0, 0 and `pair_v` (x 0, y 0) of
    // `base_g`, `base_v` 0 on `base_g` and on the inheritance parent that `fb_g` reaches, and `own_v` 0.0.
    // `base_g` and `fb_g` hold no value until the engine creates them, so they read Null. A store that wrote
    // no defaults would read Null in every declared cell and fail the first assertion.
    let fixture = Fixture::constructed();
    let reader = fixture.reader();
    assert_eq!(
        reader.read_global("dint_g").map(value_of),
        Some(Value::DInt(0))
    );
    assert_eq!(
        reader.read_global("real_g").map(value_of),
        Some(Value::Real(0.0))
    );
    assert_eq!(
        reader.read_global("text_g").map(value_of),
        Some(Value::String(SmolStr::new("")))
    );
    assert_eq!(
        reader.read_global("wide_g").map(value_of),
        Some(Value::WString(String::new()))
    );
    assert_eq!(
        reader.read_global("ref_g").map(value_of),
        Some(Value::Reference(None))
    );
    assert_eq!(
        reader.read_global("base_g").map(value_of),
        Some(Value::Null)
    );
    assert_eq!(reader.read_global("fb_g").map(value_of), Some(Value::Null));
    assert_eq!(
        reader
            .read_instance_var(BASE_G_INSTANCE, "base_v")
            .map(value_of),
        Some(Value::DInt(0))
    );
    assert_eq!(
        reader
            .read_instance_var(BASE_G_INSTANCE, "series_v")
            .map(value_of),
        Some(zero_series())
    );
    assert_eq!(
        reader
            .read_instance_var(BASE_G_INSTANCE, "pair_v")
            .map(value_of),
        Some(zero_pair())
    );
    assert_eq!(
        reader
            .read_instance_field(DERIVED_INSTANCE, "base_v")
            .ok()
            .map(value_of),
        Some(Value::DInt(0))
    );
    assert_eq!(
        reader
            .read_instance_field(DERIVED_INSTANCE, "own_v")
            .ok()
            .map(value_of),
        Some(Value::Real(0.0))
    );
}

#[test]
fn per_call_and_persistent_identities_never_collide() {
    // Specification 34 §6.2 (R4): one identity counter serves both kinds of instance. The fixture's
    // persistent instances hold 0, 1 and 2, so a per-call instance takes 3 and the next one 4. A per-call
    // identity drawn from the `Value` half's own counter, which starts at 0, would be 0: the identity of
    // the parent instance, whose members a read through the new instance would then return.
    let mut fixture = Fixture::new();
    let mut writer = CellWriter::new(&fixture.prepared, &mut fixture.arm, &mut fixture.half);
    let first = StoreMutate::try_create_instance(&mut writer, "Helper");
    let second = StoreMutate::try_create_instance(&mut writer, "Helper");
    assert_eq!(first, Ok(InstanceId(3)));
    assert_eq!(second, Ok(InstanceId(4)));
}

/// The bytes that `CellStorage::construct` asks its charge for, in its one request.
fn construction_charge(prepared: &PreparedModule) -> usize {
    let requested = std::cell::Cell::new(None);
    CellStorage::construct(prepared, Generation::FIRST, |bytes| {
        assert_eq!(requested.replace(Some(bytes)), None, "one request");
        Ok(())
    })
    .expect("the persistent region reserves");
    requested.get().expect("construction asks for its charge")
}

#[test]
fn the_persistent_instance_table_is_charged_before_it_is_reserved() {
    // Specification 34 §6.2 and O18 (C1-9d2b-2, R5): construction charges the instance table, one 28-byte
    // entry for each root that has a template POU, in the one charge it makes before it reserves anything.
    // The fixture has four such roots (the program, `base_g`, `fb_g` and the parent of `fb_g`'s instance).
    // The fixture without that parent's root has three, and the same cells, text and leaves: the parent's
    // range lies inside `fb_g`'s, so removing its root removes only a `roots` table entry (12 bytes) and an
    // instance table entry (28 bytes). The two charges therefore differ by 12 + 28 = 40 bytes. A construction
    // that reserved the table without charging it would differ by 12.
    let with_parent = construction_charge(&prepared());
    let without_parent = construction_charge(&prepared_without_parent_root());
    assert_eq!(with_parent - without_parent, 40);
}

#[test]
fn the_persistent_instance_table_never_grows() {
    // Specification 34 §6.2 (C1-9d2b-2, R5): the instance table is reserved once, from the number of roots
    // that have a template POU, and never grows. The fixture has four such roots, so it holds room for four
    // instances: four identities are recorded, recording one of them again replaces it, and a fifth is
    // refused with `InvalidExecutionState` and changes nothing. A table that grew with its entries would
    // admit the fifth.
    let prepared = prepared();
    let cells = CellStorage::construct(&prepared, Generation::FIRST, |_| Ok(()))
        .expect("the persistent region reserves");
    let mut arm = CellArm::new(cells);
    assert_eq!(arm.cells().regions().instance_capacity(), 4);
    let range = CellRange {
        region: LayoutRegion::Persistent,
        start: 0,
        cells: 0,
    };
    for identity in 0..4 {
        assert_eq!(
            arm.register_instance(InstanceId(identity), BASE_POU, range, None),
            Ok(())
        );
    }
    assert_eq!(
        arm.register_instance(InstanceId(2), DERIVED_POU, range, None),
        Ok(())
    );
    assert_eq!(arm.persistent_count(), 4);
    assert_eq!(
        arm.register_instance(InstanceId(4), BASE_POU, range, None),
        Err(RuntimeError::InvalidExecutionState)
    );
    assert_eq!(arm.persistent_count(), 4);
    assert_eq!(arm.cells().regions().instance_capacity(), 4);
}
