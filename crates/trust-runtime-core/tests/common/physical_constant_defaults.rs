//! Admission of compound constants must establish the same backing as construction.
use super::*;
use trust_runtime_core::bytecode::{
    compute_type_offsets_for_entries, ConstEntry, Field, TypeData, TypeEntry, TypeKind,
};
use trust_runtime_core::value::{RefSegment, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn fixture(low: u16, array: bool) -> BytecodeModule {
    let mut module = default_constant_bounds::owned_default();
    let SectionData::StringTable(strings) = module.section_mut(SectionId::StringTable).unwrap()
    else {
        panic!("strings")
    };
    let name = strings.entries.len() as u32;
    strings
        .entries
        .extend(["Overlay".into(), "wide".into(), "low".into()]);
    let SectionData::TypeTable(types) = module.section_mut(SectionId::TypeTable).unwrap() else {
        panic!("types")
    };
    let word = types.entries.len() as u32;
    for id in [3, 4] {
        types.entries.push(TypeEntry {
            kind: TypeKind::Primitive,
            name_idx: None,
            data: TypeData::Primitive {
                prim_id: id,
                max_length: 0,
            },
        });
    }
    let mut ty = types.entries.len() as u32;
    types.entries.push(TypeEntry {
        kind: TypeKind::OverlappingStruct,
        name_idx: Some(name),
        data: TypeData::Struct {
            fields: vec![
                Field {
                    name_idx: name + 1,
                    type_id: word + 1,
                    bit_offset: Some(0),
                },
                Field {
                    name_idx: name + 2,
                    type_id: word,
                    bit_offset: Some(0),
                },
            ],
        },
    });
    let mut payload = 2_u32.to_le_bytes().to_vec();
    for child in [
        0xaabb1234_u32.to_le_bytes().to_vec(),
        low.to_le_bytes().to_vec(),
    ] {
        payload.extend_from_slice(&(child.len() as u32).to_le_bytes());
        payload.extend_from_slice(&child);
    }
    if array {
        types.entries.push(TypeEntry {
            kind: TypeKind::Array,
            name_idx: None,
            data: TypeData::Array {
                elem_type_id: ty,
                dims: vec![(0, 0)],
            },
        });
        ty += 1;
        let mut wrapped = 1_u32.to_le_bytes().to_vec();
        wrapped.extend_from_slice(&(payload.len() as u32).to_le_bytes());
        wrapped.extend_from_slice(&payload);
        payload = wrapped;
    }
    types.offsets = compute_type_offsets_for_entries(&types.entries).unwrap();
    let SectionData::ConstPool(constants) = module.section_mut(SectionId::ConstPool).unwrap()
    else {
        panic!("constants")
    };
    constants.entries = vec![ConstEntry {
        type_id: ty,
        payload,
    }];
    let SectionData::StorageLayout(layout) = module.section_mut(SectionId::StorageLayout).unwrap()
    else {
        panic!("layout")
    };
    layout.entries[0].type_id = Some(ty);
    layout.entries[0].default_const_idx = Some(0);
    layout.entries[0].construction_nodes = if array { 4 } else { 3 };
    let SectionData::VarMeta(meta) = module.section_mut(SectionId::VarMeta).unwrap() else {
        panic!("metadata")
    };
    for entry in &mut meta.entries {
        entry.type_id = ty;
        if entry.init_const_idx.is_some() {
            entry.init_const_idx = Some(0);
        }
    }
    module
}

#[test]
fn serialized_zero_body_defaults_have_physical_aliases_including_array_members() {
    for array in [false, true] {
        let module = fixture(0x1234, array);
        module.validate().unwrap();
        let prepared =
            PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
                .unwrap();
        let mut state = prepared.instantiate(0).unwrap();
        let mut path = Vec::new();
        if array {
            path.push(RefSegment::Index(vec![0]));
        }
        path.push(RefSegment::Field("low".into()));
        state
            .write_named_path("payload", &path, Value::Word(0x5678))
            .unwrap();
        let mut value = state.storage().get_global("payload").unwrap();
        if let Value::Array(array) = value {
            value = &array.elements()[0];
        }
        let Value::Struct(value) = value else {
            panic!("physical structure")
        };
        assert_eq!(value.field("wide"), Some(&Value::DWord(0xaabb5678)));
    }
}

#[test]
fn contradictory_serialized_alias_constants_are_rejected_before_instantiation() {
    for array in [false, true] {
        let module = fixture(0xffff, array);
        // Structural validation alone is insufficient; materialization must
        // check the actual physical values before publishing a prepared module.
        module.validate().unwrap();
        assert!(PreparedModule::from_bytes(
            &module.encode().unwrap(),
            PreparationLimits::default()
        )
        .is_err());
    }
}
