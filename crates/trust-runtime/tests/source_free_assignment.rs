//! Ordinary engineering/VM assignments validate aggregates without invoking defaults.
use std::sync::Arc;
use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::collections::OrderedMap;
use trust_runtime_core::value::{ArrayValue, EnumValue, StructValue, Value};
use trust_runtime_core::vm::{PreparationLimits, PreparedModule};

fn prepare() -> PreparedModule {
    let source = r#"
TYPE
    Pair : STRUCT label : STRING[3]; number : INT; END_STRUCT;
    Mode : (Disabled, Enabled);
    RefBox : STRUCT target : REF_TO INT; END_STRUCT;
    PairAlias : Pair;
    Small : INT (1..4);
    Choice : UNION count : INT; ready : BOOL; END_UNION;
END_TYPE
VAR_GLOBAL
    item : Pair;
    items : ARRAY[1..2] OF Pair;
    mode_value : Mode;
    refs : RefBox;
    alias_item : PairAlias;
    small_value : Small;
    choice_value : Choice;
    integer_target : INT;
    bool_target : BOOL;
END_VAR
PROGRAM Main END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default()).unwrap()
}
fn structure(name: &str, fields: &[(&str, Value)]) -> Value {
    let mut members = OrderedMap::default();
    for (name, value) in fields {
        members.insert((*name).into(), value.clone());
    }
    Value::Struct(Arc::new(StructValue::from_canonical_parts(
        name.into(),
        members,
    )))
}
fn pair(label: &str, number: Value) -> Value {
    structure(
        "Pair",
        &[("label", Value::String(label.into())), ("number", number)],
    )
}

#[test]
fn malformed_aggregate_writes_leave_the_destination_unchanged() {
    let prepared = prepare();
    let mut state = prepared.instantiate(0).unwrap();
    let before = state.storage().get_global("item").unwrap().clone();
    for value in [
        Value::Bool(true),
        structure(
            "Other",
            &[
                ("label", Value::String("x".into())),
                ("number", Value::Int(1)),
            ],
        ),
        structure("Pair", &[("label", Value::String("x".into()))]),
        pair("x", Value::Bool(true)),
        structure(
            "Pair",
            &[
                ("label", Value::String("x".into())),
                ("LABEL", Value::String("y".into())),
            ],
        ),
    ] {
        assert!(state.write_global("item", value).is_err());
        assert_eq!(state.storage().get_global("item"), Some(&before));
    }
    let before = state.storage().get_global("items").unwrap().clone();
    for (values, dimensions) in [
        (vec![pair("x", Value::Int(1))], vec![(1, 2)]),
        (
            vec![pair("x", Value::Int(1)), pair("y", Value::Int(2))],
            vec![(0, 1)],
        ),
        (
            vec![pair("x", Value::Int(1)), Value::Bool(true)],
            vec![(1, 2)],
        ),
    ] {
        assert!(state
            .write_global(
                "items",
                Value::Array(Box::new(ArrayValue::from_canonical_parts(
                    values, dimensions
                )))
            )
            .is_err());
        assert_eq!(state.storage().get_global("items"), Some(&before));
    }
}

#[test]
fn nested_strings_are_normalized_and_enum_identity_is_closed() {
    let prepared = prepare();
    let mut state = prepared.instantiate(0).unwrap();
    state
        .write_global("item", pair("abcdef", Value::Int(4)))
        .unwrap();
    assert_eq!(
        state.storage().get_global("item"),
        Some(&pair("abc", Value::Int(4)))
    );
    let on = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Enabled".into(),
        1,
    )));
    state.write_global("mode_value", on.clone()).unwrap();
    assert_eq!(state.storage().get_global("mode_value"), Some(&on));
    let before = state.storage().get_global("mode_value").unwrap().clone();
    for value in [
        Value::Int(1),
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Other".into(),
            "Enabled".into(),
            1,
        ))),
        Value::Enum(Box::new(EnumValue::from_canonical_parts(
            "Mode".into(),
            "Disabled".into(),
            1,
        ))),
    ] {
        assert!(state.write_global("mode_value", value).is_err());
        assert_eq!(state.storage().get_global("mode_value"), Some(&before));
    }
}

#[test]
fn nested_references_keep_destination_type_compatibility() {
    let prepared = prepare();
    let mut state = prepared.instantiate(0).unwrap();
    let wrong = state.storage().ref_for_global("bool_target").unwrap();
    let before = state.storage().get_global("refs").unwrap().clone();
    assert!(state
        .write_global(
            "refs",
            structure("RefBox", &[("target", Value::Reference(Some(wrong)))])
        )
        .is_err());
    assert_eq!(state.storage().get_global("refs"), Some(&before));
    let right = state.storage().ref_for_global("integer_target").unwrap();
    let expected = structure("RefBox", &[("target", Value::Reference(Some(right)))]);
    state.write_global("refs", expected.clone()).unwrap();
    assert_eq!(state.storage().get_global("refs"), Some(&expected));
}

#[test]
fn vm_aggregate_assignment_does_not_evaluate_member_defaults_again() {
    let source = r#"
TYPE DivisorType : INT := INT#1; END_TYPE
VAR_GLOBAL Divisor : DivisorType; END_VAR
TYPE Payload : STRUCT number : INT := 10 / 1; END_STRUCT; END_TYPE
VAR_GLOBAL Original : Payload; CopyValue : Payload; END_VAR
PROGRAM Main
VAR_EXTERNAL Divisor : DivisorType; Original : Payload; CopyValue : Payload; END_VAR
Divisor := 0;
CopyValue := Original;
END_PROGRAM
"#;
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    use trust_runtime::bytecode::{InitializerBodyKind, SectionData, SectionId};
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let Some(SectionData::StorageLayout(layout)) = module.section(SectionId::StorageLayout) else {
        panic!("layout")
    };
    let divisor = layout
        .entries
        .iter()
        .find(|entry| strings.entries[entry.name_idx as usize] == "Divisor")
        .unwrap()
        .ref_idx
        .unwrap();
    let Some(SectionData::Initializers(index)) = module.section(SectionId::Initializers) else {
        panic!("initializers")
    };
    let recipes: Vec<_> = index
        .entries
        .iter()
        .filter(|entry| entry.body_kind == InitializerBodyKind::MemberDefault)
        .map(|entry| (entry.code_offset, entry.code_length))
        .collect();
    assert!(!recipes.is_empty());
    let Some(SectionData::PouBodies(code)) = module.section_mut(SectionId::PouBodies) else {
        panic!("code")
    };
    for (offset, length) in recipes {
        let body = &mut code[offset as usize..(offset + length) as usize];
        let loads: Vec<_> = body
            .windows(6)
            .enumerate()
            .filter(|(_, bytes)| bytes[0] == 0x10 && bytes[5] == 0x43)
            .map(|(offset, _)| offset)
            .collect();
        assert_eq!(
            loads.len(),
            1,
            "one literal denominator immediately precedes DIV"
        );
        // Source admits the constant initializer. This broader wire fixture reads
        // a mutable global to prove ordinary assignment never replays defaults.
        body[loads[0]] = 0x20;
        body[loads[0] + 1..loads[0] + 5].copy_from_slice(&divisor.to_le_bytes());
    }
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    // The type default establishes one during the resource-default pass, before
    // Payload defaults run. Explicit global initializers would be too late.
    assert_eq!(state.storage().get_global("Divisor"), Some(&Value::Int(1)));
    for name in ["Original", "CopyValue"] {
        assert_eq!(
            state.storage().get_global(name),
            Some(&structure("Payload", &[("number", Value::Int(10))]))
        );
    }

    state
        .execute_cycle(trust_runtime_core::value::Duration::from_millis(10))
        .unwrap();
    assert_eq!(state.storage().get_global("Divisor"), Some(&Value::Int(0)));
    assert_eq!(
        state.storage().get_global("CopyValue"),
        Some(&structure("Payload", &[("number", Value::Int(10))]))
    );
}

#[test]
fn aliases_subranges_and_union_variants_use_the_same_assignment_gate() {
    let prepared = prepare();
    let mut state = prepared.instantiate(0).unwrap();
    state
        .write_global("alias_item", pair("abcdef", Value::Int(4)))
        .unwrap();
    assert_eq!(
        state.storage().get_global("alias_item"),
        Some(&pair("abc", Value::Int(4)))
    );
    state.write_global("small_value", Value::Int(4)).unwrap();
    assert!(state.write_global("small_value", Value::Int(5)).is_err());
    assert_eq!(
        state.storage().get_global("small_value"),
        Some(&Value::Int(4))
    );
    let valid = structure(
        "Choice",
        &[("count", Value::Int(7)), ("ready", Value::Bool(true))],
    );
    state.write_global("choice_value", valid.clone()).unwrap();
    assert!(state
        .write_global(
            "choice_value",
            structure("Choice", &[("count", Value::Int(8))])
        )
        .is_err());
    assert_eq!(state.storage().get_global("choice_value"), Some(&valid));
}

#[test]
fn aggregate_out_and_inout_arguments_keep_typed_copyback() {
    let source = r#"
TYPE Payload : STRUCT label : STRING[3]; number : INT; END_STRUCT; END_TYPE
VAR_GLOBAL Original : Payload; CopyValue : Payload; END_VAR
FUNCTION Transform : INT
VAR_IN_OUT changed : Payload; END_VAR
VAR_OUTPUT copied : Payload; END_VAR
copied := changed;
changed.number := 2;
Transform := 1;
END_FUNCTION
PROGRAM Main
VAR_EXTERNAL Original : Payload; CopyValue : Payload; END_VAR
Transform(changed := Original, copied => CopyValue);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state
        .write_global(
            "Original",
            structure(
                "Payload",
                &[
                    ("label", Value::String("abcdef".into())),
                    ("number", Value::Int(4)),
                ],
            ),
        )
        .unwrap();
    state
        .execute_cycle(trust_runtime_core::value::Duration::from_millis(10))
        .unwrap();
    assert_eq!(
        state.storage().get_global("Original"),
        Some(&structure(
            "Payload",
            &[
                ("label", Value::String("abc".into())),
                ("number", Value::Int(2))
            ]
        ))
    );
    assert_eq!(
        state.storage().get_global("CopyValue"),
        Some(&structure(
            "Payload",
            &[
                ("label", Value::String("abc".into())),
                ("number", Value::Int(4))
            ]
        ))
    );
}

#[test]
fn bounded_string_normalization_reports_budget_failure_before_commit() {
    let module = CompileSession::from_source(
        "VAR_GLOBAL text : STRING[3]; END_VAR PROGRAM Main END_PROGRAM",
    )
    .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
    .unwrap();
    let limits = PreparationLimits {
        max_construction_bytes: 1024 * 1024,
        max_work: 8 * 1024 * 1024,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(&module.encode().unwrap(), limits).unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    let before = state.storage().get_global("text").unwrap().clone();
    let error = state
        .write_global("text", Value::String("x".repeat(2 * 1024 * 1024).into()))
        .unwrap_err();
    assert_eq!(
        error,
        trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error()
    );
    assert_eq!(state.storage().get_global("text"), Some(&before));
}

#[test]
fn char_output_copyback_can_target_a_synthesized_string_element() {
    let source = r#"
FUNCTION EmitChar : INT
VAR_INPUT seed : STRING[1]; END_VAR
VAR_OUTPUT ch : CHAR; END_VAR
ch := seed[1];
EmitChar := 1;
END_FUNCTION
PROGRAM Main
VAR output_text : STRING[3] := 'abc'; result : INT; END_VAR
result := EmitChar(seed := 'Z', ch => output_text[1]);
END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state
        .execute_cycle(trust_runtime_core::value::Duration::from_millis(10))
        .unwrap();
    let Some(Value::Instance(instance)) = state.storage().get_global("Main") else {
        panic!("program instance")
    };
    assert_eq!(
        state.storage().get_instance_var(*instance, "output_text"),
        Some(&Value::String("Zbc".into()))
    );
    assert_eq!(
        state.storage().get_instance_var(*instance, "result"),
        Some(&Value::Int(1))
    );
}

#[test]
fn reference_to_inherited_global_instance_member_has_its_declared_type() {
    let source = r#"
CLASS Base
VAR PUBLIC number : INT := INT#7; END_VAR
END_CLASS
CLASS Derived EXTENDS Base END_CLASS
VAR_GLOBAL device : Derived; target_ref : REF_TO INT; END_VAR
PROGRAM Main target_ref := REF(device.number); END_PROGRAM
"#;
    let module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let prepared =
        PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
            .unwrap();
    let mut state = prepared.instantiate(0).unwrap();
    state
        .execute_cycle(trust_runtime_core::value::Duration::from_millis(10))
        .unwrap();
    let mut expected = state.storage().ref_for_global("device").unwrap();
    expected
        .path
        .push(trust_runtime_core::value::RefSegment::Field(
            "number".into(),
        ));
    assert_eq!(
        state.storage().get_global("target_ref"),
        Some(&Value::Reference(Some(expected.clone())))
    );
    assert_eq!(
        state.storage().read_by_ref_ref(&expected),
        Some(&Value::Int(7))
    );
}

fn program_root_with_external_type_shadow() -> PreparedModule {
    use trust_runtime::bytecode::{SectionData, SectionId, StorageOwner, StorageRole};
    let source = "VAR_GLOBAL target_ref : REF_TO INT; boolean_value : BOOL; END_VAR PROGRAM Main VAR number : INT := INT#7; END_VAR END_PROGRAM";
    let mut module = CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .unwrap();
    let Some(SectionData::StringTable(strings)) = module.section(SectionId::StringTable) else {
        panic!("strings")
    };
    let number_name = strings
        .entries
        .iter()
        .position(|name| name == "number")
        .unwrap() as u32;
    let bool_name = strings
        .entries
        .iter()
        .position(|name| name == "boolean_value")
        .unwrap() as u32;
    let Some(SectionData::StorageLayout(layout)) = module.section_mut(SectionId::StorageLayout)
    else {
        panic!("layout")
    };
    let boolean_type = layout
        .entries
        .iter()
        .find(|entry| entry.owner == StorageOwner::Global && entry.name_idx == bool_name)
        .unwrap()
        .type_id;
    let mut alias = layout
        .entries
        .iter()
        .find(|entry| entry.owner == StorageOwner::Instance && entry.name_idx == number_name)
        .unwrap()
        .clone();
    alias.role = StorageRole::External;
    alias.type_id = boolean_type;
    alias.default_const_idx = None;
    alias.flags = 0;
    alias.retain = 0;
    alias.construction_nodes = 0;
    // A valid non-owning record deliberately precedes the real INT member.
    layout.entries.insert(0, alias);
    for entry in &mut layout.entries {
        if let Some(index) = &mut entry.related_declaration_idx {
            *index += 1;
        }
    }
    let Some(SectionData::ConstructionRoots(roots)) =
        module.section_mut(SectionId::ConstructionRoots)
    else {
        panic!("roots")
    };
    for root in &mut roots.entries {
        root.declaration_idx += 1;
    }
    let Some(SectionData::Initializers(initializers)) = module.section_mut(SectionId::Initializers)
    else {
        panic!("initializers")
    };
    for entry in &mut initializers.entries {
        if let Some(index) = &mut entry.declaration_idx {
            *index += 1;
        }
    }
    PreparedModule::from_bytes(&module.encode().unwrap(), PreparationLimits::default())
        .expect("external shadow owns no physical member")
}

#[test]
fn reference_to_program_root_member_ignores_external_type_shadow() {
    use trust_runtime_core::value::RefSegment;
    let prepared = program_root_with_external_type_shadow();
    let mut state = prepared.instantiate(0).unwrap();
    let mut reference = state.storage().ref_for_global("Main").unwrap();
    reference.path.push(RefSegment::Field("number".into()));
    state
        .write_global("target_ref", Value::Reference(Some(reference.clone())))
        .unwrap();
    assert_eq!(
        state.storage().get_global("target_ref"),
        Some(&Value::Reference(Some(reference.clone())))
    );
    assert_eq!(
        state.storage().read_by_ref_ref(&reference),
        Some(&Value::Int(7))
    );
    let wrong = state.storage().ref_for_global("boolean_value").unwrap();
    assert_eq!(
        state.write_global("target_ref", Value::Reference(Some(wrong))),
        Err(trust_runtime_core::error::RuntimeError::TypeMismatch)
    );
    assert_eq!(
        state.storage().get_global("target_ref"),
        Some(&Value::Reference(Some(reference)))
    );
}
