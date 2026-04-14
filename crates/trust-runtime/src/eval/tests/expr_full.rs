use super::common;

use trust_hir::types::TypeRegistry;
use trust_runtime::eval::eval_expr;
use trust_runtime::eval::expr::Expr;
use trust_runtime::harness::TestHarness;
use trust_runtime::memory::VariableStorage;
use trust_runtime::value::Value;

#[test]
fn iec_7_3_2() {
    let source = r#"
        FUNCTION Add : INT
        VAR_INPUT
            a : INT;
            b : INT;
        END_VAR
        Add := a + b;
        END_FUNCTION

        FUNCTION_BLOCK Counter
        VAR
            count : INT := INT#5;
        END_VAR
        VAR_OUTPUT
            value : INT;
        END_VAR
        value := THIS.count;
        END_FUNCTION_BLOCK

        PROGRAM Test
        VAR
            x : INT := INT#2;
            y : INT := INT#3;
            sum : INT := 0;
            neg : INT := 0;
            idx : INT := INT#1;
            arr : ARRAY[1..3] OF INT;
            arr_val : INT := 0;
            ref_out : INT := 0;
            fb_out : INT := 0;
            fb : Counter;
            r : REF_TO INT;
            size_t : DINT := 0;
        END_VAR
        arr[idx] := INT#7;
        arr_val := arr[idx];
        sum := Add(x, y);
        neg := -(sum);
        size_t := SIZEOF(INT);
        r := REF(x);
        ref_out := r^;
        fb(value => fb_out);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "unexpected runtime errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("sum", 5i16);
    harness.assert_eq("neg", -5i16);
    harness.assert_eq("arr_val", 7i16);
    harness.assert_eq("ref_out", 2i16);
    harness.assert_eq("fb_out", 5i16);
    harness.assert_eq("size_t", 2i32);
}

#[test]
fn super_uses_parent_instance() {
    let mut storage = VariableStorage::new();
    let parent = storage.create_instance("Base");
    let child = storage.create_instance("Derived");
    storage.get_instance_mut(child).unwrap().parent = Some(parent);

    let registry = TypeRegistry::new();
    let mut ctx = common::make_context(&mut storage, &registry);
    ctx.current_instance = Some(child);

    let value = eval_expr(&mut ctx, &Expr::Super).unwrap();
    assert_eq!(value, Value::Instance(parent));
}
