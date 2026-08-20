use super::*;

use std::fmt::Debug;

use crate::io::IoSize;
use crate::memory::{IoArea, MemoryLocation};
use crate::value::{RefSegment, ValueRef};
use trust_hir::types::{StructField, UnionVariant};

fn assert_compile_error<T: Debug>(result: Result<T, CompileError>, expected: &str) {
    let message = result.expect_err("expected direct I/O error").to_string();
    assert!(
        message
            .to_ascii_lowercase()
            .contains(&expected.to_ascii_lowercase()),
        "expected {message:?} to contain {expected:?}"
    );
}

fn reference() -> ValueRef {
    ValueRef {
        location: MemoryLocation::Global,
        offset: 7,
        path: Vec::new(),
    }
}

fn field(name: &str, type_id: TypeId, address: Option<&str>) -> StructField {
    StructField {
        name: name.into(),
        type_id,
        address: address.map(Into::into),
        default_initializer: None,
    }
}

fn variant(name: &str, type_id: TypeId, address: Option<&str>) -> UnionVariant {
    UnionVariant {
        name: name.into(),
        type_id,
        address: address.map(Into::into),
        default_initializer: None,
    }
}

#[test]
fn harness_io_contract_elementary_types_map_to_exact_io_widths() {
    let registry = TypeRegistry::new();
    for (type_id, expected) in [
        (TypeId::BOOL, IoSize::Bit),
        (TypeId::SINT, IoSize::Byte),
        (TypeId::USINT, IoSize::Byte),
        (TypeId::BYTE, IoSize::Byte),
        (TypeId::CHAR, IoSize::Byte),
        (TypeId::INT, IoSize::Word),
        (TypeId::UINT, IoSize::Word),
        (TypeId::WORD, IoSize::Word),
        (TypeId::WCHAR, IoSize::Word),
        (TypeId::DINT, IoSize::DWord),
        (TypeId::UDINT, IoSize::DWord),
        (TypeId::DWORD, IoSize::DWord),
        (TypeId::REAL, IoSize::DWord),
        (TypeId::TIME, IoSize::DWord),
        (TypeId::DATE, IoSize::DWord),
        (TypeId::TOD, IoSize::DWord),
        (TypeId::DT, IoSize::DWord),
        (TypeId::LINT, IoSize::LWord),
        (TypeId::ULINT, IoSize::LWord),
        (TypeId::LWORD, IoSize::LWord),
        (TypeId::LREAL, IoSize::LWord),
        (TypeId::LTIME, IoSize::LWord),
        (TypeId::LDATE, IoSize::LWord),
        (TypeId::LTOD, IoSize::LWord),
        (TypeId::LDT, IoSize::LWord),
    ] {
        assert_eq!(io_size_for_type(type_id, &registry).unwrap(), expected);
    }
}

#[test]
fn harness_io_contract_alias_subrange_and_enum_use_storage_width() {
    let mut registry = TypeRegistry::new();
    let alias = registry.register(
        "Counter",
        Type::Alias {
            name: "Counter".into(),
            target: TypeId::DINT,
        },
    );
    let subrange = registry.register(
        "Small",
        Type::Subrange {
            base: TypeId::UINT,
            lower: 0,
            upper: 10,
        },
    );
    let enumeration = registry.register_enum("Mode", TypeId::BYTE, vec![("Auto".into(), 1)]);

    assert_eq!(io_size_for_type(alias, &registry).unwrap(), IoSize::DWord);
    assert_eq!(io_size_for_type(subrange, &registry).unwrap(), IoSize::Word);
    assert_eq!(
        io_size_for_type(enumeration, &registry).unwrap(),
        IoSize::Byte
    );
    assert_eq!(leaf_value_type(alias, &registry).unwrap(), TypeId::DINT);
    assert_eq!(leaf_value_type(subrange, &registry).unwrap(), TypeId::UINT);
    assert_eq!(
        leaf_value_type(enumeration, &registry).unwrap(),
        TypeId::BYTE
    );
}

#[test]
fn harness_io_contract_bounded_string_uses_declared_byte_capacity() {
    let mut registry = TypeRegistry::new();
    let string = registry.register_string_with_length(13);
    assert_eq!(
        io_size_for_type(string, &registry).unwrap(),
        IoSize::Bytes(13)
    );
}

#[test]
fn harness_io_contract_unbounded_and_unsupported_types_fail_closed() {
    let mut registry = TypeRegistry::new();
    let unbounded = registry.register("Text", Type::String { max_len: None });
    let wstring = registry.register_wstring_with_length(8);
    let structure = registry.register_struct("Record", vec![]);
    let pointer = registry.register_pointer(TypeId::INT);
    for type_id in [unbounded, wstring, structure, pointer, TypeId::UNKNOWN] {
        assert_compile_error(
            io_size_for_type(type_id, &registry),
            if type_id == unbounded {
                "requires string"
            } else {
                "unsupported type"
            },
        );
    }
}

#[test]
fn harness_io_contract_type_size_delegates_checked_portable_layout() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::WORD, vec![(1, 3)]);
    assert_eq!(type_size_bytes(TypeId::BOOL, &registry).unwrap(), 1);
    assert_eq!(type_size_bytes(array, &registry).unwrap(), 6);
    let unbounded = registry.register("Text", Type::String { max_len: None });
    assert_compile_error(type_size_bytes(unbounded, &registry), "unsupported size");
}

#[test]
fn harness_io_contract_absolute_field_addresses_preserve_area_and_shape() {
    for (text, area, size, byte, bit) in [
        ("%IX1.2", IoArea::Input, IoSize::Bit, 1, 2),
        ("%QW4", IoArea::Output, IoSize::Word, 4, 0),
        ("%MD8", IoArea::Memory, IoSize::DWord, 8, 0),
    ] {
        let FieldAddress::Absolute(address) = parse_field_address(text).unwrap() else {
            panic!("expected absolute address");
        };
        assert_eq!(address.area, area);
        assert_eq!(address.size, size);
        assert_eq!(address.byte, byte);
        assert_eq!(address.bit, bit);
    }
}

#[test]
fn harness_io_contract_relative_field_addresses_preserve_offsets() {
    for (text, expected_bytes, expected_bit) in [
        ("%X0", 0, 0),
        ("%X2.7", 2, 7),
        ("%B3", 3, 0),
        ("%W4", 4, 0),
        ("%D8", 8, 0),
        ("%L16", 16, 0),
    ] {
        let FieldAddress::Relative {
            offset_bytes,
            bit_offset,
        } = parse_field_address(text).unwrap()
        else {
            panic!("expected relative address");
        };
        assert_eq!(offset_bytes, expected_bytes);
        assert_eq!(bit_offset, expected_bit);
    }
}

#[test]
fn harness_io_contract_field_address_grammar_is_case_sensitive_and_closed() {
    for text in [
        "", "X0", "%", "%x0", "%b0", "%X", "%X.", "%X1.", "%X.1", "%X1.8", "%X1.2.3", "%B",
        "%B1.0", "%Z0", "%IX",
    ] {
        assert_compile_error(parse_field_address(text), "invalid");
    }
}

#[test]
fn harness_io_contract_field_address_trims_surrounding_whitespace() {
    let FieldAddress::Relative {
        offset_bytes,
        bit_offset,
    } = parse_field_address(" \t%X2.3\n").unwrap()
    else {
        panic!("expected relative address");
    };
    assert_eq!((offset_bytes, bit_offset), (2, 3));
}

#[test]
fn harness_io_contract_nonbit_offsets_advance_simple_address() {
    let base = IoAddress::parse("%QW10").unwrap();
    let address = offset_address(&base, 4, IoSize::DWord, 0).unwrap();
    assert_eq!(address.area, IoArea::Output);
    assert_eq!(address.size, IoSize::DWord);
    assert_eq!(address.byte, 14);
    assert_eq!(address.path, vec![14]);
    assert_eq!(address.bit, 0);
    assert!(!address.wildcard);
}

#[test]
fn harness_io_contract_bit_offsets_carry_across_bytes() {
    let base = IoAddress::parse("%IX10.3").unwrap();
    let address = offset_address(&base, 2, IoSize::Bit, 6).unwrap();
    assert_eq!(address.byte, 13);
    assert_eq!(address.path, vec![13]);
    assert_eq!(address.bit, 1);
}

#[test]
fn harness_io_contract_hierarchical_offsets_advance_only_final_component() {
    let base = IoAddress::parse("%IX1.2.3").unwrap();
    let bit = offset_address(&base, 1, IoSize::Bit, 6).unwrap();
    assert_eq!(bit.byte, 1);
    assert_eq!(bit.path, vec![1, 4]);
    assert_eq!(bit.bit, 1);

    let base = IoAddress::parse("%IW1.2.3").unwrap();
    let word = offset_address(&base, 4, IoSize::Word, 0).unwrap();
    assert_eq!(word.byte, 1);
    assert_eq!(word.path, vec![1, 2, 7]);
    assert_eq!(word.bit, 0);
}

#[test]
fn harness_io_contract_offset_arithmetic_rejects_every_overflow() {
    let simple = IoAddress::parse("%QW4294967295").unwrap();
    assert_compile_error(
        offset_address(&simple, 1, IoSize::Word, 0),
        "offset overflow",
    );

    let hierarchical = IoAddress::parse("%QW1.4294967295").unwrap();
    assert_compile_error(
        offset_address(&hierarchical, 1, IoSize::Word, 0),
        "offset overflow",
    );
    assert_compile_error(
        offset_address(
            &IoAddress::parse("%QW0").unwrap(),
            u64::MAX,
            IoSize::Word,
            0,
        ),
        "offset overflow",
    );
}

#[test]
fn harness_io_contract_primitive_binding_preserves_reference_width_and_type() {
    let registry = TypeRegistry::new();
    let mut out = Vec::new();
    collect_io_bindings(&registry, TypeId::DINT, reference(), 5, 0, &mut out).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].reference, reference());
    assert_eq!(out[0].offset_bytes, 5);
    assert_eq!(out[0].bit_offset, 0);
    assert_eq!(out[0].size, IoSize::DWord);
    assert_eq!(out[0].value_type, TypeId::DINT);
}

#[test]
fn harness_io_contract_nonbool_leaf_rejects_bit_offset() {
    let registry = TypeRegistry::new();
    assert_compile_error(
        collect_io_bindings(&registry, TypeId::WORD, reference(), 0, 1, &mut Vec::new()),
        "bit offset only allowed for bool",
    );
}

#[test]
fn harness_io_contract_array_binding_is_row_major_with_declared_indices() {
    let mut registry = TypeRegistry::new();
    let array = registry.register_array(TypeId::WORD, vec![(1, 2), (-1, 0)]);
    let mut out = Vec::new();
    collect_io_bindings(&registry, array, reference(), 0, 0, &mut out).unwrap();

    assert_eq!(
        out.iter()
            .map(|binding| binding.offset_bytes)
            .collect::<Vec<_>>(),
        vec![0, 2, 4, 6]
    );
    assert_eq!(
        out.iter()
            .map(|binding| binding.reference.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![RefSegment::Index(vec![1, -1])],
            vec![RefSegment::Index(vec![1, 0])],
            vec![RefSegment::Index(vec![2, -1])],
            vec![RefSegment::Index(vec![2, 0])],
        ]
    );
}

#[test]
fn harness_io_contract_struct_binding_is_sequential_with_relative_overrides() {
    let mut registry = TypeRegistry::new();
    let structure = registry.register_struct(
        "Record",
        vec![
            field("flag", TypeId::BOOL, Some("%X0.3")),
            field("count", TypeId::WORD, Some("%W2")),
            field("tail", TypeId::BYTE, None),
        ],
    );
    let mut out = Vec::new();
    collect_io_bindings(&registry, structure, reference(), 10, 0, &mut out).unwrap();

    assert_eq!(out.len(), 3);
    assert_eq!((out[0].offset_bytes, out[0].bit_offset), (10, 3));
    assert_eq!((out[1].offset_bytes, out[1].bit_offset), (12, 0));
    assert_eq!((out[2].offset_bytes, out[2].bit_offset), (14, 0));
    assert_eq!(
        out.iter()
            .map(|binding| binding.reference.path.clone())
            .collect::<Vec<_>>(),
        vec![
            vec![RefSegment::Field("flag".into())],
            vec![RefSegment::Field("count".into())],
            vec![RefSegment::Field("tail".into())],
        ]
    );
}

#[test]
fn harness_io_contract_union_variants_overlay_unless_explicitly_offset() {
    let mut registry = TypeRegistry::new();
    let union = registry.register_union(
        "Choice",
        vec![
            variant("word", TypeId::WORD, None),
            variant("flag", TypeId::BOOL, Some("%X1.2")),
        ],
    );
    let mut out = Vec::new();
    collect_io_bindings(&registry, union, reference(), 5, 0, &mut out).unwrap();

    assert_eq!(out.len(), 2);
    assert_eq!((out[0].offset_bytes, out[0].bit_offset), (5, 0));
    assert_eq!((out[1].offset_bytes, out[1].bit_offset), (6, 2));
}

#[test]
fn harness_io_contract_absolute_nested_field_is_rejected_with_base_binding() {
    let mut registry = TypeRegistry::new();
    let structure =
        registry.register_struct("Record", vec![field("flag", TypeId::BOOL, Some("%IX0.0"))]);
    assert_compile_error(
        collect_io_bindings(&registry, structure, reference(), 0, 0, &mut Vec::new()),
        "absolute direct address not allowed",
    );
}

#[test]
fn harness_io_contract_bounded_string_binding_and_unsupported_leaves_are_closed() {
    let mut registry = TypeRegistry::new();
    let string = registry.register_string_with_length(8);
    let mut out = Vec::new();
    collect_io_bindings(&registry, string, reference(), 3, 0, &mut out).unwrap();
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].size, IoSize::Bytes(8));
    assert_eq!(out[0].value_type, TypeId::STRING);

    let unbounded = registry.register("Text", Type::String { max_len: None });
    let wstring = registry.register_wstring_with_length(8);
    let pointer = registry.register_pointer(TypeId::INT);
    for type_id in [unbounded, wstring, pointer] {
        assert_compile_error(
            collect_io_bindings(&registry, type_id, reference(), 0, 0, &mut Vec::new()),
            if type_id == unbounded {
                "requires string"
            } else {
                "not supported"
            },
        );
    }
}

#[test]
fn harness_io_contract_join_instance_path_handles_root_and_nested_names() {
    assert_eq!(
        join_instance_path(&SmolStr::new(""), &SmolStr::new("leaf")),
        "leaf"
    );
    assert_eq!(
        join_instance_path(&SmolStr::new("root"), &SmolStr::new("leaf")),
        "root.leaf"
    );
}
