//! Saved artifact: nested structure and array defaults are materialized as runtime values at every
//! cold initialization (spec 34 §5.4 LOAD-11).

use trust_runtime::harness::CompileSession;
use trust_runtime::value::Value;
use trust_runtime::{RestartMode, Runtime};

fn global(runtime: &Runtime, name: &str) -> Value {
    runtime
        .storage()
        .get_global(name)
        .cloned()
        .unwrap_or_else(|| panic!("missing global {name}"))
}

fn struct_field(value: &Value, name: &str) -> Value {
    let Value::Struct(struct_value) = value else {
        panic!("expected a structure, got {value:?}");
    };
    struct_value
        .fields()
        .get(name)
        .cloned()
        .unwrap_or_else(|| panic!("missing structure field {name}"))
}

fn array_elements(value: &Value) -> Vec<Value> {
    let Value::Array(array) = value else {
        panic!("expected an array, got {value:?}");
    };
    array.elements().to_vec()
}

fn assert_declared_defaults(runtime: &Runtime) {
    let pair = global(runtime, "pair");
    assert_eq!(struct_field(&pair, "first"), Value::Int(3));
    assert_eq!(struct_field(&pair, "second"), Value::Int(9));
    let rows = array_elements(&global(runtime, "rows"));
    assert_eq!(rows.len(), 3);
    for row in &rows {
        assert_eq!(struct_field(row, "first"), Value::Int(7));
        assert_eq!(struct_field(row, "second"), Value::Int(4));
    }
}

#[test]
fn saved_nested_structure_and_array_defaults_return_after_a_cold_restart() {
    let source = r#"
TYPE
    Setting : STRUCT first : INT := 3; second : INT := 4; END_STRUCT;
END_TYPE
VAR_GLOBAL
    pair : Setting := (second := 9);
    rows : ARRAY[1..3] OF Setting := [3((first := 7))];
END_VAR
PROGRAM Main
pair.first := INT#100;
rows[1].second := INT#50;
END_PROGRAM
"#;
    let bytes = CompileSession::from_source(source)
        .build_bytecode_module()
        .unwrap()
        .encode()
        .unwrap();
    let mut runtime =
        Runtime::from_bytes(&bytes, Default::default()).expect("saved artifact constructs state");
    assert_declared_defaults(&runtime);

    runtime.execute_cycle().unwrap();
    assert_eq!(
        struct_field(&global(&runtime, "pair"), "first"),
        Value::Int(100)
    );
    let rows = array_elements(&global(&runtime, "rows"));
    assert_eq!(struct_field(&rows[0], "second"), Value::Int(50));

    runtime.restart(RestartMode::Cold).unwrap();
    assert_declared_defaults(&runtime);
}
