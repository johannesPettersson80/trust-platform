//! Incomplete formal calls of the standard functions of fixed inputs (IEC 61131-3 Ed.3 §6.6.1.4.2;
//! specification 07 §1, "Calls"; docs/IEC_DECISIONS.md 2026-11-21): an omitted input takes the
//! default value of the type its call gives it, and a call that gives no input typing an omitted
//! one is refused with an error naming the inputs it needs.

use crate::common::*;

/// The errors of `statement` in a program with a variable of each type the calls take.
fn errors(statement: &str) -> Vec<String> {
    let mut db = Database::new();
    let file = FileId(0);
    db.set_source_text(
        file,
        format!(
            "PROGRAM Main\nVAR a : INT; b : INT; r : REAL; w : WORD; t : TIME; ok : BOOL; \
             s : STRING; END_VAR\n{statement}\nEND_PROGRAM"
        ),
    );
    db.diagnostics(file)
        .iter()
        .filter(|diagnostic| diagnostic.severity == DiagnosticSeverity::Error)
        .map(|diagnostic| diagnostic.message.clone())
        .collect()
}

/// The refusal of a formal call of `function` that omits `inputs`, which type it.
fn untyped(function: &str, inputs: &str) -> String {
    format!(
        "{function} needs {inputs} to give its type: an input that a formal call omits takes the \
         default value of that type"
    )
}

#[test]
fn a_formal_call_of_fixed_inputs_may_omit_inputs_that_its_other_inputs_type() {
    // Specification 07 §1: an omitted input takes the type of the inputs given with it, or its own
    // (SEL's selector, a shift's count, EXPT's exponent), and so is accepted; LIMIT(IN := b,
    // MX := 5) is IEC's own example of an incomplete call. Catches the refusal "expected 3
    // arguments, found 2" that truST gave each of them (checklist G227).
    for statement in [
        "a := LIMIT(IN := b, MX := 5);",
        "a := LIMIT(MN := b);",
        "t := LIMIT(IN := t, MX := T#2s);",
        "s := LIMIT(IN := s, MX := 'k');",
        "r := ATAN2(Y := r);",
        "r := ATAN2(X := REAL#1.0);",
        "a := SUB(IN1 := b);",
        "a := SUB(IN2 := b);",
        "a := DIV(IN2 := b);",
        "a := MOD(IN1 := b);",
        "r := EXPT(IN1 := r);",
        "w := SHL(IN := w);",
        "w := ROR(IN := w);",
        "a := SEL(IN1 := b);",
        "a := SEL(G := ok, IN0 := b);",
        "s := SEL(G := ok, IN0 := s);",
        "ok := NE(IN1 := b);",
        "ok := NE(IN2 := s);",
    ] {
        assert_eq!(errors(statement), Vec::<String>::new(), "{statement}");
    }
}

#[test]
fn a_formal_call_that_gives_no_input_typing_an_omitted_one_is_refused_naming_the_inputs() {
    // Specification 07 §1: with every input of the generic omitted, a shift's value or EXPT's base,
    // an omitted input has no type, so no default value: the call is refused, once, naming the
    // inputs it needs. Catches a call that picks a type of its own for the default, and one
    // accepted without a type, which left the call's type unknown.
    for (statement, function, inputs) in [
        ("a := LIMIT();", "LIMIT", "MN, IN or MX"),
        ("a := SEL(G := ok);", "SEL", "IN0 or IN1"),
        ("w := SHL(N := 2);", "SHL", "IN"),
        ("w := ROL(N := 2);", "ROL", "IN"),
        ("r := EXPT(IN2 := 2);", "EXPT", "IN1"),
        ("r := ATAN2();", "ATAN2", "Y or X"),
        ("ok := NE();", "NE", "IN1 or IN2"),
        ("a := SUB();", "SUB", "IN1 or IN2"),
        ("a := DIV();", "DIV", "IN1 or IN2"),
        ("a := MOD();", "MOD", "IN1 or IN2"),
    ] {
        assert_eq!(
            errors(statement),
            [untyped(function, inputs)],
            "{statement}"
        );
    }
}

#[test]
fn sub_and_div_of_a_time_take_both_inputs() {
    // Specification 07 §1: SUB and DIV of a time are overloaded (IEC Table 37), so the type of an
    // omitted input depends on the overload and it has no default value. Catches a call that took
    // the given time's type for the omitted input, so that SUB(IN1 := t) of a DATE or a DT gave
    // one of several results.
    for (statement, function) in [
        ("t := SUB(IN1 := t);", "SUB"),
        ("t := SUB(IN2 := t);", "SUB"),
        ("t := DIV(IN1 := t);", "DIV"),
    ] {
        assert_eq!(
            errors(statement),
            [format!(
                "{function} of a time needs IN1 and IN2: the type of an omitted input depends on \
                 the other's (IEC 61131-3 Ed.3 Table 37)"
            )],
            "{statement}"
        );
    }
}

#[test]
fn a_non_formal_call_still_gives_every_input() {
    // IEC 61131-3 Ed.3 §6.6.1.4.2: a non-formal call has "exactly the same number of parameters"
    // as the function, so only a formal call may be incomplete. Catches the incomplete formal
    // call's acceptance reaching a positional one.
    for (statement, message) in [
        ("a := LIMIT(b, 5);", "expected 3 arguments, found 2"),
        ("w := SHL(w);", "expected 2 arguments, found 1"),
        ("ok := NE(b);", "expected 2 arguments, found 1"),
    ] {
        assert_eq!(errors(statement), [message], "{statement}");
    }
}
