use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[test]
fn timing_diagrams() {
    // IEC 61131-3:2013, Table 46 timing behavior for TON/TOF/TP.
    let source = r#"
        PROGRAM Test
        VAR
            ton : TON;
            tof : TOF;
            tp : TP;
            in_ton : BOOL;
            in_tof : BOOL;
            in_tp : BOOL;
            pt : TIME := T#10ms;
            q_ton : BOOL; et_ton : TIME;
            q_tof : BOOL; et_tof : TIME;
            q_tp : BOOL; et_tp : TIME;
        END_VAR
        ton(IN := in_ton, PT := pt, Q => q_ton, ET => et_ton);
        tof(IN := in_tof, PT := pt, Q => q_tof, ET => et_tof);
        tp(IN := in_tp, PT := pt, Q => q_tp, ET => et_tp);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    // t = 0ms, all inputs low.
    harness.set_input("in_ton", false);
    harness.set_input("in_tof", false);
    harness.set_input("in_tp", false);
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));

    // Rising edge at t = 5ms.
    harness.set_input("in_ton", true);
    harness.set_input("in_tof", true);
    harness.set_input("in_tp", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(5)));

    // t = 10ms, TON done, TP pulse ends.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(true));
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));

    // Falling edge at t = 15ms.
    harness.set_input("in_ton", false);
    harness.set_input("in_tof", false);
    harness.set_input("in_tp", false);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));

    // t = 20ms, TOF delay expires.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));

    // t = 25ms, TOF ET resets.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));

    // New TP pulse after re-trigger.
    harness.set_input("in_tp", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(5)));
}
