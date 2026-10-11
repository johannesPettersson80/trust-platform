use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::value::{size_of_value_with, ArrayValue, EnumValue, SizeOfError, Value};

fn size(value: &Value) -> Result<u64, SizeOfError> {
    size_of_value_with(
        value,
        &mut |_| Err(SizeOfError::UnsupportedType),
        &mut |_| Ok(()),
    )
}
#[test]
fn runtime_size_preserves_character_count_and_first_array_element_rule() {
    assert_eq!(size(&Value::String("åβ".into())), Ok(2));
    assert_eq!(size(&Value::WString("åβ".into())), Ok(4));
    let array = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![Value::String("a".into()), Value::String("longer".into())],
        vec![(5, 6)],
    )));
    assert_eq!(size(&array), Ok(2));
    assert_eq!(
        size(&Value::Reference(None)),
        Ok(core::mem::size_of::<usize>() as u64)
    );
    assert_eq!(size(&Value::Null), Err(SizeOfError::UnsupportedType));
}
#[test]
fn enum_size_uses_metadata_callback_and_preserves_callback_fault() {
    let value = Value::Enum(Box::new(EnumValue::from_canonical_parts(
        "Mode".into(),
        "Run".into(),
        2,
    )));
    let mut names = Vec::new();
    let result: Result<u64, SizeOfError> = size_of_value_with(
        &value,
        &mut |name| {
            names.push(name.to_owned());
            Ok(2)
        },
        &mut |_| Ok(()),
    );
    assert_eq!(result, Ok(2));
    assert_eq!(names, vec!["Mode"]);
    let error = trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error();
    let result: Result<u64, RuntimeError> =
        size_of_value_with(&value, &mut |_| Ok(2), &mut |_| Err(error.clone()));
    assert_eq!(result, Err(error));
}
#[test]
fn runtime_value_size_accepts_128_nested_nodes_and_rejects_the_first_excess() {
    let mut value = Value::Int(7);
    for _ in 0..127 {
        value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
            vec![value],
            vec![(0, 0)],
        )));
    }
    assert_eq!(size(&value), Ok(2));
    value = Value::Array(Box::new(ArrayValue::from_canonical_parts(
        vec![value],
        vec![(0, 0)],
    )));
    assert_eq!(size(&value), Err(SizeOfError::UnsupportedType));
}

#[test]
fn type_size_shared_graph_charges_each_visit_and_preserves_exhaustion_error() {
    use trust_runtime_core::bytecode::{Field, TypeData, TypeEntry, TypeKind, TypeTable};
    let types = TypeTable {
        offsets: vec![],
        entries: vec![
            TypeEntry {
                kind: TypeKind::Primitive,
                name_idx: None,
                data: TypeData::Primitive {
                    prim_id: 7,
                    max_length: 0,
                },
            },
            TypeEntry {
                kind: TypeKind::Struct,
                name_idx: None,
                data: TypeData::Struct {
                    fields: vec![
                        Field {
                            name_idx: 0,
                            type_id: 0,
                        },
                        Field {
                            name_idx: 1,
                            type_id: 0,
                        },
                    ],
                },
            },
        ],
    };
    let mut work = 0;
    assert_eq!(
        trust_runtime_core::vm::sizeof_type_from_table_with(&types, 1, &mut |units| {
            work += units;
            Ok(())
        }),
        Ok(4)
    );
    assert!(work >= 3, "both repeated primitive edges must consume work");
    let error = trust_runtime_core::vm::VmTrap::BudgetExceeded.into_runtime_error();
    let mut left = 1usize;
    assert_eq!(
        trust_runtime_core::vm::sizeof_type_from_table_with(&types, 1, &mut |units| {
            left = left.checked_sub(units).ok_or_else(|| error.clone())?;
            Ok(())
        }),
        Err(error)
    );
}
