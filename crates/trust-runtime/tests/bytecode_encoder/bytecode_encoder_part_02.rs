use super::*;

#[test]
fn encoder_emits_composite_types() {
    let source = r#"
TYPE
    MySubrange : INT(0..10);
    MyAlias : INT;
    MyArray : ARRAY[1..3] OF INT;
    MyStruct : STRUCT
        a : INT;
        b : BOOL;
    END_STRUCT;
    MyUnion : UNION
        u1 : INT;
        u2 : BOOL;
    END_UNION;
    MyEnum : (Red := 1, Green := 2, Blue := 3);
    MyNamed : INT (Low := 1, High := 2);
    MyRef : REF_TO INT;
END_TYPE

PROGRAM Main
VAR
    sr : MySubrange;
    al : MyAlias;
    arr : MyArray;
    st : MyStruct;
    un : MyUnion;
    enum_val : MyEnum;
    named_val : MyNamed;
    rf : MyRef;
END_VAR
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let types = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table,
        other => panic!("expected TYPE_TABLE, got {other:?}"),
    };

    let array_alias = find_type(types, strings, "MyArray");
    assert_eq!(array_alias.kind, TypeKind::Alias);
    let array_type_id = if let TypeData::Alias { target_type_id } = &array_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let array = types
        .entries
        .get(array_type_id as usize)
        .expect("array target type");
    assert_eq!(array.kind, TypeKind::Array);
    if let TypeData::Array { elem_type_id, dims } = &array.data {
        assert_eq!(dims, &vec![(1, 3)]);
        expect_primitive(types, *elem_type_id, 7);
    } else {
        panic!("expected array type data");
    }

    let strukt = find_type(types, strings, "MyStruct");
    assert_eq!(strukt.kind, TypeKind::Struct);
    if let TypeData::Struct { fields } = &strukt.data {
        assert_eq!(fields.len(), 2);
        assert_eq!(lookup_string(strings, fields[0].name_idx), "a");
        assert_eq!(lookup_string(strings, fields[1].name_idx), "b");
        expect_primitive(types, fields[0].type_id, 7);
        expect_primitive(types, fields[1].type_id, 1);
    } else {
        panic!("expected struct type data");
    }

    let union = find_type(types, strings, "MyUnion");
    assert_eq!(union.kind, TypeKind::Union);
    if let TypeData::Union { fields } = &union.data {
        assert_eq!(fields.len(), 2);
        assert_eq!(lookup_string(strings, fields[0].name_idx), "u1");
        assert_eq!(lookup_string(strings, fields[1].name_idx), "u2");
        expect_primitive(types, fields[0].type_id, 7);
        expect_primitive(types, fields[1].type_id, 1);
    } else {
        panic!("expected union type data");
    }

    let enum_ty = find_type(types, strings, "MyEnum");
    assert_eq!(enum_ty.kind, TypeKind::Enum);
    if let TypeData::Enum {
        base_type_id,
        variants,
    } = &enum_ty.data
    {
        expect_primitive(types, *base_type_id, 7);
        assert_eq!(variants.len(), 3);
        assert_eq!(lookup_string(strings, variants[0].name_idx), "Red");
        assert_eq!(lookup_string(strings, variants[1].name_idx), "Green");
        assert_eq!(lookup_string(strings, variants[2].name_idx), "Blue");
        assert_eq!(variants[0].value, 1);
        assert_eq!(variants[1].value, 2);
        assert_eq!(variants[2].value, 3);
    } else {
        panic!("expected enum type data");
    }

    let named_value = find_type(types, strings, "MyNamed");
    assert_eq!(named_value.kind, TypeKind::Alias);
    if let TypeData::Alias { target_type_id } = &named_value.data {
        expect_primitive(types, *target_type_id, 7);
    } else {
        panic!("expected named-value alias type data");
    }

    let alias = find_type(types, strings, "MyAlias");
    assert_eq!(alias.kind, TypeKind::Alias);
    if let TypeData::Alias { target_type_id } = &alias.data {
        expect_primitive(types, *target_type_id, 7);
    } else {
        panic!("expected alias type data");
    }

    let subrange_alias = find_type(types, strings, "MySubrange");
    assert_eq!(subrange_alias.kind, TypeKind::Alias);
    let subrange_type_id = if let TypeData::Alias { target_type_id } = &subrange_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let subrange = types
        .entries
        .get(subrange_type_id as usize)
        .expect("subrange target type");
    assert_eq!(subrange.kind, TypeKind::Subrange);
    if let TypeData::Subrange {
        base_type_id,
        lower,
        upper,
    } = &subrange.data
    {
        expect_primitive(types, *base_type_id, 7);
        assert_eq!(*lower, 0);
        assert_eq!(*upper, 10);
    } else {
        panic!("expected subrange type data");
    }

    let reference_alias = find_type(types, strings, "MyRef");
    assert_eq!(reference_alias.kind, TypeKind::Alias);
    let reference_type_id = if let TypeData::Alias { target_type_id } = &reference_alias.data {
        *target_type_id
    } else {
        panic!("expected alias type data");
    };
    let reference = types
        .entries
        .get(reference_type_id as usize)
        .expect("reference target type");
    assert_eq!(reference.kind, TypeKind::Reference);
    if let TypeData::Reference { target_type_id } = &reference.data {
        expect_primitive(types, *target_type_id, 7);
    } else {
        panic!("expected reference type data");
    }
}

#[test]
fn encoder_emits_interface_methods() {
    let source = r#"
INTERFACE IBase
METHOD Foo : INT
END_METHOD
END_INTERFACE

INTERFACE IDerived EXTENDS IBase
METHOD Bar : INT
END_METHOD
END_INTERFACE

CLASS Impl IMPLEMENTS IDerived
METHOD PUBLIC Foo : INT
Foo := INT#1;
END_METHOD
METHOD PUBLIC Bar : INT
Bar := INT#2;
END_METHOD
END_CLASS

PROGRAM Main
VAR
    i : IDerived;
    b : IBase;
    c : Impl;
END_VAR
i := c;
END_PROGRAM
"#;

    let module = bytecode_module_from_source(source).unwrap();
    let strings = match module.section(SectionId::StringTable) {
        Some(SectionData::StringTable(table)) => table,
        other => panic!("expected STRING_TABLE, got {other:?}"),
    };
    let types = match module.section(SectionId::TypeTable) {
        Some(SectionData::TypeTable(table)) => table,
        other => panic!("expected TYPE_TABLE, got {other:?}"),
    };

    let base = find_type(types, strings, "IBase");
    assert_eq!(base.kind, TypeKind::Interface);
    if let TypeData::Interface { methods } = &base.data {
        expect_interface_methods(methods, strings, &["Foo"]);
    } else {
        panic!("expected interface type data");
    }

    let derived = find_type(types, strings, "IDerived");
    assert_eq!(derived.kind, TypeKind::Interface);
    if let TypeData::Interface { methods } = &derived.data {
        expect_interface_methods(methods, strings, &["Foo", "Bar"]);
    } else {
        panic!("expected interface type data");
    }
}
