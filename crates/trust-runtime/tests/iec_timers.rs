use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

#[test]
fn timing_diagrams() {
    // IEC 61131-3:2013, Table 46 timing behavior for TON/TOF/TP, observed at calls 5 ms apart
    // (specification 08 §5; docs/IEC_DECISIONS.md 2026-10-30): the call that samples the starting
    // edge gives ET 0 and each later call adds 5 ms, as CODESYS and TwinCAT time.
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

    // Rising edge at t = 5ms: the edge call starts TON and TP at ET 0.
    harness.set_input("in_ton", true);
    harness.set_input("in_tof", true);
    harness.set_input("in_tp", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));

    // t = 10ms, 5 ms since the edge: TON still below PT, the TP pulse runs.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::from_millis(5)));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(5)));

    // Falling edge at t = 15ms: TON resets, the TOF delay starts at ET 0, and the TP pulse ends
    // on this call, 10 ms after its edge, keeping ET at PT.
    harness.set_input("in_ton", false);
    harness.set_input("in_tof", false);
    harness.set_input("in_tp", false);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_ton", Value::Bool(false));
    harness.assert_eq("et_ton", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::ZERO));
    harness.assert_eq("q_tp", Value::Bool(false));
    harness.assert_eq("et_tp", Value::Time(Duration::from_millis(10)));

    // t = 20ms, 5 ms into the TOF delay.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(true));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(5)));

    // t = 25ms, the TOF delay expires with ET at PT.
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));

    // t = 30ms: the TOF's ET holds at PT until rearm, and a new TP edge starts a pulse at ET 0.
    harness.set_input("in_tp", true);
    harness.advance_time(Duration::from_millis(5));
    harness.cycle();
    harness.assert_eq("q_tof", Value::Bool(false));
    harness.assert_eq("et_tof", Value::Time(Duration::from_millis(10)));
    harness.assert_eq("q_tp", Value::Bool(true));
    harness.assert_eq("et_tp", Value::Time(Duration::ZERO));
}
