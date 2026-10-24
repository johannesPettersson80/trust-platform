//! `debug.evaluate`, `eval` and the options of `breakpoints.set` on a compiled program
//! (specification 11 §6.13.2, version 1), over the fixture of the parent module, through the shared
//! dispatcher: the names of the held frame and of its instance, the globals and the program
//! instances' variables, each line's condition, hit condition and logpoint message, and the
//! refusals with their codes.

use super::*;
use alloc::boxed::Box;
use base64::Engine as _;
use smol_str::SmolStr;
use trust_runtime_core::program_model::BinaryOp;

use crate::debug::expression::{encode, Expression};
use crate::protocol::ControlRequest;

fn name(text: &str) -> Expression {
    Expression::Name(SmolStr::new(text))
}

fn int(value: i16) -> Expression {
    Expression::Literal(Value::Int(value))
}

fn binary(op: BinaryOp, left: Expression, right: Expression) -> Expression {
    Expression::Binary(op, Box::new(left), Box::new(right))
}

fn field(target: Expression, member: &str) -> Expression {
    Expression::Field(Box::new(target), SmolStr::new(member))
}

/// The answer to the request `kind` with `params` from the program `held` holds.
fn ask(held: &Held, kind: &str, params: Json) -> ControlResponse {
    let request = ControlRequest {
        id: 1,
        r#type: String::from(kind),
        params: Some(params),
        auth: None,
        request_id: None,
    };
    crate::debug::requests::answer(&request, held)
}

/// The answer to `debug.evaluate` of `expression`, compiled, in frame `frame_id`.
fn evaluate(held: &Held, expression: &Expression, frame_id: Option<u32>) -> ControlResponse {
    let bytes = encode(expression).expect("version 1 carries it");
    ask(
        held,
        "debug.evaluate",
        json!({
            "expr_version": 1,
            "expr": base64::engine::general_purpose::STANDARD.encode(bytes),
            "frame_id": frame_id,
        }),
    )
}

/// The answer a value gets, as `debug.variables` reports a variable of its type.
fn answer(value: &str, type_name: &str) -> Json {
    json!({ "result": value, "type": type_name, "variables_reference": 0 })
}

/// The stable code of a refusal.
fn code(response: &ControlResponse) -> &str {
    assert!(!response.ok, "{:?}", response.result);
    response.error_code.as_deref().expect("a code")
}

#[test]
fn between_scans_names_are_globals_and_the_program_instances_variables() {
    // Specification 11 §6.13.2: with no frame, a name is a global or a program instance's
    // variable, and the operators compute as statements do. Worked from the fixture: `Limit + 1`
    // is 8 (INT); `Plant.Speed * 2` is 10; `Origin.Y` is -4; `Levels[2]` is the third element, 30;
    // `Plant.Ctr.Count + Total` is 40 + 100 = 140, a DINT. Catches a name read from another area or
    // offset, a member of an instance or a structure that is never reached, an index from one, and
    // a result reported in another form than `debug.variables` reports it.
    let held = Held::new(Vec::new());
    let cases = [
        (
            binary(BinaryOp::Add, name("Limit"), int(1)),
            answer("8", "INT"),
        ),
        (
            binary(BinaryOp::Mul, field(name("Plant"), "Speed"), int(2)),
            answer("10", "INT"),
        ),
        (field(name("Origin"), "Y"), answer("-4", "INT")),
        (
            Expression::Index(Box::new(name("Levels")), vec![int(2)]),
            answer("30", "INT"),
        ),
        (
            binary(
                BinaryOp::Add,
                field(field(name("PLANT"), "ctr"), "Count"),
                name("total"),
            ),
            answer("140", "DINT"),
        ),
    ];
    for (expression, expected) in cases {
        assert_eq!(
            result(evaluate(&held, &expression, None)),
            expected,
            "{expression:?}"
        );
    }
}

#[test]
fn in_a_held_frame_its_own_variables_come_first_then_its_instances() {
    // Specification 11 §6.13.2: in a held scan a name is first a variable of the frame asked for:
    // its own (a function's inputs and locals), then its instance's. The stack holds `SumTo`
    // (frame 2) called from `Main` on `Plant` (frame 1). Worked: in frame 2, `N * 2` is 12 * 2 =
    // 24 and `Tally.Count`, a member of the function's own instance, is 60; in frame 1,
    // `Speed + Ctr.Count` is 5 + 40 = 45, a DINT; `N` is no variable of frame 1 nor a global, so
    // `expr_invalid`; frame 3 does not exist. Catches a frame counted from the innermost, a frame
    // variable read from an area, the members of an instance in a frame never reached, and a name
    // of one frame read in another.
    let held = Held::new(vec![sum_to(2), main_body(1)]);
    assert_eq!(
        result(evaluate(
            &held,
            &binary(BinaryOp::Mul, name("N"), int(2)),
            Some(2)
        )),
        answer("24", "INT")
    );
    assert_eq!(
        result(evaluate(&held, &field(name("Tally"), "Count"), Some(2))),
        answer("60", "DINT")
    );
    assert_eq!(
        result(evaluate(
            &held,
            &binary(BinaryOp::Add, name("Speed"), field(name("Ctr"), "Count")),
            Some(1)
        )),
        answer("45", "DINT")
    );
    assert_eq!(code(&evaluate(&held, &name("N"), Some(1))), "expr_invalid");
    assert_eq!(
        error(&evaluate(&held, &name("N"), Some(3))),
        "unknown frame id"
    );
}

#[test]
fn refusals_carry_their_stable_codes() {
    // Specification 11 §6.13.2: another version is `expr_version_unsupported` and decodes nothing;
    // bytes that are not base64 and a name that is an instance, which holds no value, are
    // `expr_invalid`; `Levels[3]`, past the bound 2, is `expr_evaluation_failed`. Catches a
    // version-2 form decoded as version 1, and a refusal under another code or a value made up.
    let held = Held::new(Vec::new());
    let other_version = ask(
        &held,
        "debug.evaluate",
        json!({ "expr_version": 2, "expr": "AQMBAA==" }),
    );
    assert_eq!(code(&other_version), "expr_version_unsupported");
    let not_base64 = ask(
        &held,
        "debug.evaluate",
        json!({ "expr_version": 1, "expr": "not base64!" }),
    );
    assert_eq!(code(&not_base64), "expr_invalid");
    assert_eq!(
        code(&evaluate(&held, &field(name("Plant"), "Ctr"), None)),
        "expr_invalid"
    );
    assert_eq!(
        code(&evaluate(
            &held,
            &Expression::Index(Box::new(name("Levels")), vec![int(3)]),
            None
        )),
        "expr_evaluation_failed"
    );
}

#[test]
fn eval_reads_one_exact_name() {
    // Specification 11 §6.8.1: `eval` is a bounded exact name read, answered as trust-runtime
    // answers it, `{"value": "<the value's debug form>"}`. Worked: `Plant.Speed` is `Int(5)`;
    // `Missing` names no variable. Catches an `eval` refused on a compiled program, and an
    // expression parsed from its text.
    let held = Held::new(Vec::new());
    assert_eq!(
        result(ask(&held, "eval", json!({ "expr": " Plant.Speed " }))),
        json!({ "value": "Int(5)" })
    );
    assert_eq!(
        error(&ask(&held, "eval", json!({ "expr": "Missing" }))),
        "unknown identifier"
    );
}

/// A compiled expression as a request carries it.
fn wire(expression: &Expression) -> Json {
    let bytes = encode(expression).expect("version 1 carries it");
    json!({
        "expr_version": 1,
        "expr": base64::engine::general_purpose::STANDARD.encode(bytes),
    })
}

#[test]
fn breakpoints_set_gives_each_line_its_options() {
    // Specification 11 §6.13.2: `options` belong to the lines in order. The zero-based lines 13
    // and 14 of `main.st` resolve to its statements at 14:5-14:20 and 15:5-15:29; the first takes
    // the condition `Limit > 5` and the hit condition `> 2`, the second the message
    // `limit {Limit}`. Catches options given to another line, a hit condition read with another
    // operator, and a message whose parts lose their order.
    let held = Held::new(Vec::new());
    let condition = binary(BinaryOp::Gt, name("Limit"), int(5));
    let reply = ask(
        &held,
        "breakpoints.set",
        json!({
            "source": "/work/plant/main.st",
            "lines": [13, 14],
            "options": [
                {"condition": wire(&condition), "hit_condition": {"op": ">", "count": 2}},
                {"log_message": ["limit ", wire(&name("Limit"))]},
            ],
        }),
    );
    let _ = result(reply);
    let replaced = held.replaced.borrow();
    assert_eq!(replaced.len(), 1);
    let (file, breakpoints) = &replaced[0];
    assert_eq!(*file, 0);
    assert_eq!(
        *breakpoints,
        vec![
            (
                Span::new((14, 5), (14, 20)),
                crate::debug::BreakpointOptions {
                    hit_condition: Some(crate::debug::HitCondition::GreaterThan(2)),
                    condition: Some(condition),
                    log_message: None,
                },
            ),
            (
                Span::new((15, 5), (15, 29)),
                crate::debug::BreakpointOptions {
                    log_message: Some(vec![
                        crate::debug::logpoint::LogPart::Text(String::from("limit ")),
                        crate::debug::logpoint::LogPart::Expression(name("Limit")),
                    ]),
                    ..crate::debug::BreakpointOptions::default()
                },
            ),
        ]
    );
}

#[test]
fn breakpoints_set_refuses_options_outside_the_protocol_whole() {
    // Specification 11 §6.13.2: options not as many as the lines, an operator outside `=`, `>=`
    // and `>`, a count of 0, an expression of version 2 (`expr_version_unsupported`) and bytes
    // that are not base64 (`expr_invalid`) refuse the whole request, which replaces nothing.
    // Catches a request half applied, and a refusal under another code.
    let held = Held::new(Vec::new());
    let set = |options: Json| {
        ask(
            &held,
            "breakpoints.set",
            json!({"source": "/work/plant/main.st", "lines": [13], "options": options}),
        )
    };
    assert_eq!(
        error(&set(json!([null, null]))),
        "the options are not as many as the lines"
    );
    assert_eq!(
        error(&set(json!([{"hit_condition": {"op": "<", "count": 2}}]))),
        "unknown hit condition operator '<'"
    );
    assert_eq!(
        error(&set(json!([{"hit_condition": {"op": "=", "count": 0}}]))),
        "a hit condition's count is 0"
    );
    assert_eq!(
        code(&set(
            json!([{"condition": {"expr_version": 2, "expr": "AQMBAA=="}}])
        )),
        "expr_version_unsupported"
    );
    assert_eq!(
        code(&set(
            json!([{"log_message": [{"expr_version": 1, "expr": "not base64!"}]}])
        )),
        "expr_invalid"
    );
    assert_eq!(held.replaced.borrow().len(), 0);
}
