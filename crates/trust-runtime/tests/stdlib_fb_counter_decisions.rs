//! Counter edge memory follows the count input on every executed call, including
//! a call whose reset or load dominates (spec 08 counter contract).

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn ctu_edge_memory_is_updated_while_reset_dominates() {
    let source = r#"
        PROGRAM Example
        VAR
            counter : CTU;
            cu : BOOL := FALSE;
            r : BOOL := FALSE;
            q : BOOL := FALSE;
            cv : INT := 0;
        END_VAR
        counter(CU := cu, R := r, PV := INT#3, Q => q, CV => cv);
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).unwrap();

    // CU rises while R is held: the reset dominates and the rising edge is consumed.
    harness.set_input("cu", true);
    harness.set_input("r", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(0));

    // R is released while CU is still high: no new edge, so CV stays 0.
    harness.set_input("r", false);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(0));

    // A fresh rising edge counts.
    harness.set_input("cu", false);
    harness.cycle();
    harness.set_input("cu", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(1));
    harness.assert_eq("q", Value::Bool(false));
}

#[test]
fn ctd_edge_memory_is_updated_while_load_dominates() {
    let source = r#"
        PROGRAM Example
        VAR
            counter : CTD;
            cd : BOOL := FALSE;
            ld : BOOL := FALSE;
            q : BOOL := FALSE;
            cv : INT := 0;
        END_VAR
        counter(CD := cd, LD := ld, PV := INT#3, Q => q, CV => cv);
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).unwrap();

    // CD rises while LD is held: the load dominates and loads PV.
    harness.set_input("cd", true);
    harness.set_input("ld", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(3));

    // LD is released while CD is still high: no new edge, so CV stays 3.
    harness.set_input("ld", false);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(3));

    // A fresh rising edge counts down.
    harness.set_input("cd", false);
    harness.cycle();
    harness.set_input("cd", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(2));
    harness.assert_eq("q", Value::Bool(false));
}

#[test]
fn ctud_edge_memory_is_updated_while_reset_dominates() {
    let source = r#"
        PROGRAM Example
        VAR
            counter : CTUD;
            cu : BOOL := FALSE;
            cd : BOOL := FALSE;
            r : BOOL := FALSE;
            ld : BOOL := FALSE;
            qu : BOOL := FALSE;
            qd : BOOL := FALSE;
            cv : INT := 0;
        END_VAR
        counter(CU := cu, CD := cd, R := r, LD := ld, PV := INT#3, QU => qu, QD => qd, CV => cv);
        END_PROGRAM
    "#;
    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("cu", true);
    harness.set_input("r", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(0));

    harness.set_input("r", false);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(0));

    harness.set_input("cu", false);
    harness.cycle();
    harness.set_input("cu", true);
    harness.cycle();
    harness.assert_eq("cv", Value::Int(1));
}
