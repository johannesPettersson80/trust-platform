use super::*;

use crate::program_model::{BinaryOp, LValue};
use crate::value::Value;

fn registry() -> TypeRegistry {
    TypeRegistry::new()
}

#[test]
fn harness_parse_contract_watch_trims_whitespace_and_one_semicolon() {
    let Expr::Name(name) = parse_debug_expression(
        " \t value ; \n",
        &mut registry(),
        DateTimeProfile::default(),
        &[],
    )
    .unwrap() else {
        panic!("expected name");
    };
    assert_eq!(name, "value");
}

#[test]
fn harness_parse_contract_watch_rejects_empty_and_invalid_syntax() {
    for expression in ["", " ", ";", "1 +", "(", "value["] {
        assert!(
            parse_debug_expression(expression, &mut registry(), DateTimeProfile::default(), &[],)
                .is_err(),
            "{expression:?} must fail"
        );
    }
}

#[test]
fn harness_parse_contract_watch_lowers_literals_names_and_binary_expressions() {
    assert!(matches!(
        parse_debug_expression("INT#7", &mut registry(), DateTimeProfile::default(), &[],).unwrap(),
        Expr::Literal(Value::Int(7))
    ));
    assert!(matches!(
        parse_debug_expression(
            "counter",
            &mut registry(),
            DateTimeProfile::default(),
            &[],
        )
        .unwrap(),
        Expr::Name(name) if name == "counter"
    ));
    assert!(matches!(
        parse_debug_expression(
            "counter + 1",
            &mut registry(),
            DateTimeProfile::default(),
            &[],
        )
        .unwrap(),
        Expr::Binary {
            op: BinaryOp::Add,
            ..
        }
    ));
}

#[test]
fn harness_parse_contract_pure_stdlib_call_names_are_case_insensitive() {
    for name in [
        "abs", "MIN", "Max", "limit", "SEL", "mux", "sqrt", "LN", "LOG", "EXP", "sin", "COS",
        "tan", "ASIN", "acos", "ATAN", "atan2", "LEN",
    ] {
        assert!(is_allowed_watch_call(name), "{name}");
    }
}

#[test]
fn harness_parse_contract_conversion_and_time_split_calls_are_allowed() {
    for name in ["INT_TO_DINT", "BOOL_TO_STRING", "SPLIT_DT", "SPLIT_LDT"] {
        assert!(is_allowed_watch_call(name), "{name}");
    }
}

#[test]
fn harness_parse_contract_user_and_qualified_calls_are_not_allowed() {
    for name in ["DoThing", "fb.Run", "Namespace.ABS", "TON"] {
        assert!(!is_allowed_watch_call(name), "{name}");
    }
}

#[test]
fn harness_parse_contract_watch_accepts_reviewed_pure_calls() {
    for expression in ["ABS(INT#-7)", "INT_TO_DINT(INT#7)", "MAX(INT#1, INT#2)"] {
        assert!(
            parse_debug_expression(expression, &mut registry(), DateTimeProfile::default(), &[],)
                .is_ok(),
            "{expression}"
        );
    }
}

#[test]
fn harness_parse_contract_watch_rejects_user_and_method_calls() {
    for expression in ["DoThing()", "fb.Run()", "obj.child.Execute(INT#1)"] {
        let error =
            parse_debug_expression(expression, &mut registry(), DateTimeProfile::default(), &[])
                .unwrap_err()
                .to_string();
        assert!(error.contains("side-effect free"), "{error}");
    }
}

#[test]
fn harness_parse_contract_lvalue_trims_whitespace_and_one_semicolon() {
    let LValue::Name(name) = parse_debug_lvalue(
        " \t target ; \n",
        &mut registry(),
        DateTimeProfile::default(),
        &[],
    )
    .unwrap() else {
        panic!("expected name target");
    };
    assert_eq!(name, "target");
}

#[test]
fn harness_parse_contract_lvalue_rejects_empty_and_invalid_syntax() {
    for expression in ["", " ", ";", "value + 1", "(", "items["] {
        assert!(
            parse_debug_lvalue(expression, &mut registry(), DateTimeProfile::default(), &[],)
                .is_err(),
            "{expression:?} must fail"
        );
    }
}

#[test]
fn harness_parse_contract_lvalue_lowers_name_field_and_index_paths() {
    assert!(matches!(
        parse_debug_lvalue(
            "target",
            &mut registry(),
            DateTimeProfile::default(),
            &[],
        )
        .unwrap(),
        LValue::Name(name) if name == "target"
    ));
    assert!(matches!(
        parse_debug_lvalue(
            "fb.value",
            &mut registry(),
            DateTimeProfile::default(),
            &[],
        )
        .unwrap(),
        LValue::Field { field, .. } if field == "value"
    ));
    assert!(matches!(
        parse_debug_lvalue(
            "items[2]",
            &mut registry(),
            DateTimeProfile::default(),
            &[],
        )
        .unwrap(),
        LValue::Index { indices, .. } if indices.len() == 1
    ));
}

#[test]
fn harness_parse_contract_lvalue_rejects_call_in_target() {
    for expression in ["GetTarget()", "fb.GetTarget().value"] {
        let error =
            parse_debug_lvalue(expression, &mut registry(), DateTimeProfile::default(), &[])
                .unwrap_err()
                .to_string();
        assert!(error.contains("side-effect free"), "{error}");
    }
}

#[test]
fn harness_parse_contract_using_list_is_preserved_for_lowering_context() {
    let using = [SmolStr::new("Utilities"), SmolStr::new("Math")];
    assert!(parse_debug_expression(
        "counter",
        &mut registry(),
        DateTimeProfile::default(),
        &using,
    )
    .is_ok());
    assert!(parse_debug_lvalue(
        "counter",
        &mut registry(),
        DateTimeProfile::default(),
        &using,
    )
    .is_ok());
}

#[test]
fn harness_parse_contract_allowed_call_classifier_is_closed() {
    for name in ["", "ASSERT", "MOVE", "R_TRIG", "my_abs", "ABS.extra"] {
        assert!(!is_allowed_watch_call(name), "{name}");
    }
}
