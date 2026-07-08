use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[test]
fn timer_variants() {
    let source = r#"
        PROGRAM Test
        VAR
            ton : TON;
            tof : TOF;
            tp : TP;
            ton_l : TON_LTIME;
            in_ton : BOOL;
            in_tof : BOOL;
            in_tp : BOOL;
            in_ton_l : BOOL;
            pt : TIME := T#10ms;
            pt_l : LTIME := LTIME#10ms;
            q_ton : BOOL; et_ton : TIME;
            q_tof : BOOL; et_tof : TIME;
            q_tp : BOOL; et_tp : TIME;
            q_ton_l : BOOL; et_ton_l : LTIME;
        END_VAR
        ton(IN := in_ton, PT := pt, Q => q_ton, ET => et_ton);
        tof(IN := in_tof, PT := pt, Q => q_tof, ET => et_tof);
        tp(IN := in_tp, PT := pt, Q => q_tp, ET => et_tp);
        ton_l(IN := in_ton_l, PT := pt_l, Q => q_ton_l, ET => et_ton_l);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("in_ton", false);
    harness.set_input("in_tof", false);
    harness.set_input("in_tp", false);
    harness.set_input("in_ton_l", false);
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));
    harness.assert_eq("q_ton_l", Value::Bool(false));
    harness.assert_eq("et_ton_l", Value::LTime(Duration::ZERO));

    harness.set_input("in_ton", true);
    harness.set_input("in_tof", true);
    harness.set_input("in_tp", true);
    harness.set_input("in_ton_l", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_ton_l", Value::Bool(false));
    harness.assert_eq("et_ton_l", Value::LTime(Duration::from_millis(5)));

    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(true));
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_ton_l", Value::Bool(true));
    harness.assert_eq("et_ton_l", Value::LTime(Duration::from_millis(10)));

    harness.set_input("in_ton", false);
    harness.set_input("in_tof", false);
    harness.set_input("in_tp", false);
    harness.set_input("in_ton_l", false);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));
    harness.assert_eq("q_ton_l", Value::Bool(false));
    harness.assert_eq("et_ton_l", Value::LTime(Duration::ZERO));

    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));

    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));

    harness.set_input("in_tp", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(5)));
}
