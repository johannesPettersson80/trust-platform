use trust_runtime::harness::TestHarness;
use trust_runtime::value::Value;

#[test]
fn r_trig_f_trig() {
    let source = r#"
        PROGRAM Test
        VAR
            r : R_TRIG;
            f : F_TRIG;
            clk : BOOL;
            q_r : BOOL;
            q_f : BOOL;
        END_VAR
        r(CLK := clk, Q => q_r);
        f(CLK := clk, Q => q_f);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(false));
    harness.assert_eq("q_f", Value::Bool(true));

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(false));
    harness.assert_eq("q_f", Value::Bool(false));

    harness.set_input("clk", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(true));
    harness.assert_eq("q_f", Value::Bool(false));

    harness.set_input("clk", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(false));
    harness.assert_eq("q_f", Value::Bool(false));

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(false));
    harness.assert_eq("q_f", Value::Bool(true));

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_r", Value::Bool(false));
    harness.assert_eq("q_f", Value::Bool(false));
}

#[test]
fn difu_difd_aliases_match_edge_behavior() {
    let source = r#"
        PROGRAM Test
        VAR
            up : DIFU;
            down : DIFD;
            clk : BOOL;
            q_up : BOOL;
            q_down : BOOL;
        END_VAR
        up(CLK := clk, Q => q_up);
        down(CLK := clk, Q => q_down);
        END_PROGRAM
    "#;

    let mut harness = TestHarness::from_source(source).unwrap();

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_up", Value::Bool(false));
    harness.assert_eq("q_down", Value::Bool(true));

    harness.set_input("clk", Value::Bool(false));
    harness.cycle();
    harness.assert_eq("q_up", Value::Bool(false));
    harness.assert_eq("q_down", Value::Bool(false));

    harness.set_input("clk", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_up", Value::Bool(true));
    harness.assert_eq("q_down", Value::Bool(false));

    harness.set_input("clk", Value::Bool(true));
    harness.cycle();
    harness.assert_eq("q_up", Value::Bool(false));
    harness.assert_eq("q_down", Value::Bool(false));
}
