//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn counter_variants() {
    let source = r#"
        PROGRAM Test
        VAR
            ctu : CTU;
            cu_u : BOOL;
            r_u : BOOL;
            pv_u : INT := INT#2;
            q_u : BOOL;
            cv_u : INT;

            ctd : CTD;
            cd_d : BOOL;
            ld_d : BOOL;
            pv_d : INT := INT#3;
            q_d : BOOL;
            cv_d : INT;

            ctud : CTUD;
            cu_ud : BOOL;
            cd_ud : BOOL;
            r_ud : BOOL;
            ld_ud : BOOL;
            pv_ud : INT := INT#2;
            qu_ud : BOOL;
            qd_ud : BOOL;
            cv_ud : INT;

            ctd_u : CTD_UDINT;
            cd_u : BOOL;
            ld_u : BOOL;
            pv_udint : UDINT := UDINT#2;
            q_ud : BOOL;
            cv_udint : UDINT;
        END_VAR
        ctu(CU := cu_u, R := r_u, PV := pv_u, Q => q_u, CV => cv_u);
        ctd(CD := cd_d, LD := ld_d, PV := pv_d, Q => q_d, CV => cv_d);
        ctud(CU := cu_ud, CD := cd_ud, R := r_ud, LD := ld_ud, PV := pv_ud, QU => qu_ud, QD => qd_ud, CV => cv_ud);
        ctd_u(CD := cd_u, LD := ld_u, PV := pv_udint, Q => q_ud, CV => cv_udint);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("cu_u", false);
    harness.set_input("r_u", false);
    harness.set_input("cd_d", false);
    harness.set_input("ld_d", true);
    harness.set_input("cu_ud", false);
    harness.set_input("cd_ud", false);
    harness.set_input("r_ud", false);
    harness.set_input("ld_ud", false);
    harness.set_input("cd_u", false);
    harness.set_input("ld_u", true);
    harness.cycle();
    harness.assert_eq("cv_u", Value::Int(0));
    harness.assert_eq("q_u", Value::Bool(false));
    harness.assert_eq("cv_d", Value::Int(3));
    harness.assert_eq("q_d", Value::Bool(false));
    harness.assert_eq("cv_ud", Value::Int(0));
    harness.assert_eq("qu_ud", Value::Bool(false));
    harness.assert_eq("qd_ud", Value::Bool(true));
    harness.assert_eq("cv_udint", Value::UDInt(2));
    harness.assert_eq("q_ud", Value::Bool(false));

    harness.set_input("ld_d", false);
    harness.set_input("ld_u", false);

    harness.set_input("cu_u", true);
    harness.cycle();
    harness.assert_eq("cv_u", Value::Int(1));
    harness.assert_eq("q_u", Value::Bool(false));

    harness.set_input("cu_u", false);
    harness.cycle();
    harness.assert_eq("cv_u", Value::Int(1));

    harness.set_input("cu_u", true);
    harness.cycle();
    harness.assert_eq("cv_u", Value::Int(2));
    harness.assert_eq("q_u", Value::Bool(true));

    harness.set_input("cu_u", false);
    harness.cycle();

    harness.set_input("cd_d", true);
    harness.cycle();
    harness.assert_eq("cv_d", Value::Int(2));
    harness.assert_eq("q_d", Value::Bool(false));

    harness.set_input("cd_d", false);
    harness.cycle();

    harness.set_input("cd_d", true);
    harness.cycle();
    harness.assert_eq("cv_d", Value::Int(1));
    harness.assert_eq("q_d", Value::Bool(false));

    harness.set_input("cd_d", false);
    harness.cycle();

    harness.set_input("cd_d", true);
    harness.cycle();
    harness.assert_eq("cv_d", Value::Int(0));
    harness.assert_eq("q_d", Value::Bool(true));

    harness.set_input("cd_u", true);
    harness.cycle();
    harness.assert_eq("cv_udint", Value::UDInt(1));
    harness.assert_eq("q_ud", Value::Bool(false));

    harness.set_input("cd_u", false);
    harness.cycle();

    harness.set_input("cd_u", true);
    harness.cycle();
    harness.assert_eq("cv_udint", Value::UDInt(0));
    harness.assert_eq("q_ud", Value::Bool(true));

    harness.set_input("cu_ud", true);
    harness.cycle();
    harness.assert_eq("cv_ud", Value::Int(1));
    harness.assert_eq("qu_ud", Value::Bool(false));
    harness.assert_eq("qd_ud", Value::Bool(false));

    harness.set_input("cu_ud", false);
    harness.cycle();

    harness.set_input("cu_ud", true);
    harness.cycle();
    harness.assert_eq("cv_ud", Value::Int(2));
    harness.assert_eq("qu_ud", Value::Bool(true));
    harness.assert_eq("qd_ud", Value::Bool(false));

    harness.set_input("cu_ud", false);
    harness.cycle();

    harness.set_input("cd_ud", true);
    harness.cycle();
    harness.assert_eq("cv_ud", Value::Int(1));
    harness.assert_eq("qu_ud", Value::Bool(false));
    harness.assert_eq("qd_ud", Value::Bool(false));

    harness.set_input("cd_ud", false);
    harness.cycle();

    harness.set_input("r_ud", true);
    harness.cycle();
    harness.assert_eq("cv_ud", Value::Int(0));
    harness.assert_eq("qu_ud", Value::Bool(false));
    harness.assert_eq("qd_ud", Value::Bool(true));
}

/// Specification 08 §4 (IEC 61131-3 Table 45, 1a): a plain `CTU` is `CTU_INT`. A DINT preset is
/// refused when the program is compiled, and an untyped literal preset is an INT. Worked by hand,
/// with CU FALSE in scan 1 and toggled after each call: rising edges in scans 2 and 4 count CV to
/// 1 and 2, and Q rises when CV reaches PV 2 in scan 4. Catches a DINT preset accepted and run as
/// INT (a preset of 100000 ran as 0), and a literal preset refused.
#[test]
fn a_plain_counter_is_an_int_counter() {
    let refused = TestHarness::from_source(
        r#"
        PROGRAM Test
        VAR ctu : CTU; pv : DINT := DINT#100000; q : BOOL; cv : DINT; END_VAR
        ctu(CU := TRUE, R := FALSE, PV := pv, Q => q, CV => cv);
        END_PROGRAM
    "#,
    );
    let error = refused
        .err()
        .expect("a DINT preset for a plain CTU is refused")
        .to_string();
    assert!(
        error.contains("expected 'INT' for parameter 'PV'"),
        "{error}"
    );

    let mut harness = TestHarness::from_source(
        r#"
        PROGRAM Test
        VAR ctu : CTU; cu : BOOL; q : BOOL; cv : INT; END_VAR
        ctu(CU := cu, R := FALSE, PV := 2, Q => q, CV => cv);
        cu := NOT cu;
        END_PROGRAM
    "#,
    )
    .expect("a literal preset compiles");
    for (scan, cv, q) in [(1, 0, false), (2, 1, false), (3, 1, false), (4, 2, true)] {
        harness.cycle();
        harness.assert_eq("cv", Value::Int(cv));
        assert_eq!(harness.get_output("q"), Some(Value::Bool(q)), "scan {scan}");
    }
    harness.assert_eq("ctu.PV", Value::Int(2));
}
