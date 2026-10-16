//! TIME-08 STRUCT OVERLAP write cost (spec 34, "STRUCT OVERLAP write cost"): a write charges
//! 8·L·M + P units plus the per-leaf and whole-structure terms, and allocates P bytes plus one
//! leaf entry per affected leaf. Each expected value below is worked out by hand from its layout.
use super::*;
use crate::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
use crate::collections::OrderedMap;
use crate::value::{RefSegment, StorageAbi, StructValue, Value};
use alloc::{format, string::String, sync::Arc, vec, vec::Vec};
use smol_str::SmolStr;

fn primitive(id: u16) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::Primitive,
        name_idx: None,
        data: TypeData::Primitive {
            prim_id: id,
            max_length: 0,
        },
    }
}

/// A located structure whose fields sit at the given bit offsets: (name, type, bit offset).
fn overlapping_struct(fields: &[(u32, u32, u32)]) -> TypeEntry {
    TypeEntry {
        kind: TypeKind::OverlappingStruct,
        name_idx: None,
        data: TypeData::Struct {
            fields: fields
                .iter()
                .map(|&(name_idx, type_id, offset)| Field {
                    name_idx,
                    type_id,
                    bit_offset: Some(offset),
                })
                .collect(),
        },
    }
}

/// Prepares the layouts of `entries`. The names index the string table.
fn layouts(entries: Vec<TypeEntry>, names: &[&str]) -> Vec<Option<Arc<PhysicalLayout>>> {
    let types = TypeTable {
        offsets: Vec::new(),
        entries,
    };
    let names: Vec<SmolStr> = names.iter().map(SmolStr::new).collect();
    prepare_layouts(
        &types,
        &names,
        StorageAbi::UniformTemporal64,
        &mut |_, _| Ok(()),
    )
    .unwrap()
}

fn pair(fields: &[(&str, Value)], layout: Option<Arc<PhysicalLayout>>) -> Value {
    let fields = fields
        .iter()
        .map(|(name, value)| (SmolStr::new(name), value.clone()))
        .collect::<OrderedMap<_, _>>();
    Value::Struct(Arc::new(
        StructValue::from_canonical_parts("Pair".into(), fields)
            .initialize_physical_layout(layout)
            .unwrap(),
    ))
}

fn path(name: &str) -> Vec<RefSegment> {
    vec![RefSegment::Field(name.into())]
}

/// Two DWORD leaves: "a" at bit 0 and "bb" at bit 32, so the structure spans 64 bits.
/// Primitive id 4 is DWORD in the overlap layout, so its leaves hold `Value::DWord`
/// (`value/overlap/scalar.rs`, `primitive`). A DINT value would be a TypeMismatch.
fn two_leaf_layout() -> Vec<Option<Arc<PhysicalLayout>>> {
    layouts(
        vec![primitive(4), overlapping_struct(&[(0, 0, 0), (1, 0, 32)])],
        &["a", "bb"],
    )
}

#[test]
fn single_leaf_write_charges_the_formula_value() {
    // Layout facts: leaves 2, children 0. Leaf path work: w(a) = 1 segment + 1 name byte = 2;
    // w(bb) = 1 + 2 = 3, so the layout maximum is 3. Written path "a": w = 2.
    //   L = bit length of max(2, 0, 1) = bit length of 2 = 2
    //   M = max(3, 2) = 3
    //   P = bits 0..32 = 4 bytes
    //   8·L·M + P = 8·2·3 + 4 = 52
    //   selected leaf a: 192 + 4·w = 192 + 8 = 200
    //   affected leaf a (bb starts at bit 32, so it is not affected): 192 + w = 194
    //   no selected children, and the path is not empty, so no span or field-count terms
    //   total work = 52 + 200 + 194 = 446
    //   allocation = P + one leaf entry = 4 + size_of::<(usize, Value)>()
    let layout = two_leaf_layout();
    let target = pair(
        &[("a", Value::DWord(0)), ("bb", Value::DWord(0))],
        layout[1].clone(),
    );
    let Value::Struct(structure) = &target else {
        panic!("structure")
    };
    let cost = structure.physical_write_cost(&path("a")).unwrap().unwrap();
    assert_eq!(cost.work, 446);
    assert_eq!(cost.allocation, 4 + core::mem::size_of::<(usize, Value)>());
}

#[test]
fn whole_structure_write_charges_the_formula_value() {
    // Same layout, and the written path is empty: w = 0, so M = max(3, 0) = 3.
    //   P = bits 0..64 = 8 bytes
    //   8·L·M + P = 8·2·3 + 8 = 56
    //   whole structure: the selected span is charged again, so 56 + 64 + 64 = 184
    //   selected leaves: a: 192 + 4·2 = 200; bb: 192 + 4·3 = 204; total 184 + 404 = 588
    //   affected leaves: a: 192 + 2 = 194; bb: 192 + 3 = 195; total 588 + 389 = 977
    //   whole structure: field count 2, so the total work is 979
    //   allocation = P + two leaf entries = 8 + 2 × size_of::<(usize, Value)>()
    // The input carries no layout, so it is not admitted as a shared-layout assignment.
    let layout = two_leaf_layout();
    let target = pair(
        &[("a", Value::DWord(0)), ("bb", Value::DWord(0))],
        layout[1].clone(),
    );
    let Value::Struct(structure) = &target else {
        panic!("structure")
    };
    let cost = structure.physical_write_cost(&[]).unwrap().unwrap();
    assert_eq!(cost.work, 979);
    assert_eq!(
        cost.allocation,
        8 + 2 * core::mem::size_of::<(usize, Value)>()
    );
}

#[test]
fn hundred_field_single_write_charges_the_formula_value() {
    // 100 DWORD leaves "f000".."f099", leaf i at bit 32·i. The written leaf is "f050".
    //   L = bit length of 100 = 7 (100 is below 128)
    //   every name is 4 bytes, so w(f050) = 1 + 4 = 5 and the layout maximum is 5; M = 5
    //   P = bits 1600..1632 = 4 bytes (only f050 covers them)
    //   8·L·M + P = 8·7·5 + 4 = 284
    //   selected leaf: 192 + 4·5 = 212, total 496
    //   affected leaf: 192 + 5 = 197, total 693
    let names: Vec<String> = (0..100).map(|index| format!("f{index:03}")).collect();
    let labels: Vec<&str> = names.iter().map(String::as_str).collect();
    let members: Vec<(u32, u32, u32)> = (0..100u32).map(|index| (index, 0, index * 32)).collect();
    let layout = layouts(vec![primitive(4), overlapping_struct(&members)], &labels);
    let fields: Vec<(&str, Value)> = labels.iter().map(|name| (*name, Value::DWord(0))).collect();
    let target = pair(&fields, layout[1].clone());
    let Value::Struct(structure) = &target else {
        panic!("structure")
    };
    let cost = structure
        .physical_write_cost(&path("f050"))
        .unwrap()
        .unwrap();
    assert_eq!(cost.work, 693);
    assert_eq!(cost.allocation, 4 + core::mem::size_of::<(usize, Value)>());
}

#[test]
fn sparse_gap_whole_structure_write_charges_the_formula_value() {
    // Two BYTE leaves: "first" at bit 0 and "last" at bit 65528. The structure spans 65536 bits,
    // with a sparse gap between the leaves, and the written path is empty.
    //   L = bit length of max(2, 0, 1) = 2
    //   w(first) = 1 + 5 = 6 and w(last) = 1 + 4 = 5, so the layout maximum is 6; w = 0 for the
    //   whole write, so M = max(6, 0) = 6
    //   P = 65536 bits = 8192 bytes
    //   8·L·M + P = 8·2·6 + 8192 = 8288
    //   whole structure: the selected span is charged twice, 8288 + 65536 + 65536 = 139360
    //   selected leaves: first 192 + 4·6 = 216; last 192 + 4·5 = 212; total 139788
    //   affected leaves (both intersect the span): first 192 + 6 = 198; last 192 + 5 = 197;
    //   total 140183
    //   field count 2, so the total work is 140185
    //   allocation = P + two leaf entries = 8192 + 2 × size_of::<(usize, Value)>()
    let layout = layouts(
        vec![
            primitive(2),
            overlapping_struct(&[(0, 0, 0), (1, 0, 65528)]),
        ],
        &["first", "last"],
    );
    let target = pair(
        &[("first", Value::Byte(0)), ("last", Value::Byte(0))],
        layout[1].clone(),
    );
    let Value::Struct(structure) = &target else {
        panic!("structure")
    };
    let cost = structure.physical_write_cost(&[]).unwrap().unwrap();
    assert_eq!(cost.work, 140_185);
    assert_eq!(
        cost.allocation,
        8192 + 2 * core::mem::size_of::<(usize, Value)>()
    );
}
