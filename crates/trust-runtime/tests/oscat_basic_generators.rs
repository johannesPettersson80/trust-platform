use trust_runtime::execution_backend::ExecutionBackend;
use trust_runtime::harness::TestHarness;
use trust_runtime::value::{Duration, Value};

const OSCAT_BASIC_LOGIC: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_logic.st"
));
const OSCAT_BASIC_GLOBALS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_globals.st"
));
const OSCAT_BASIC_MATH: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_math.st"
));
const OSCAT_BASIC_MATH_EXTRA: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_math_extra.st"
));
const OSCAT_BASIC_CONVERSIONS: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_conversions.st"
));
const OSCAT_BASIC_STRING: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_string.st"
));
const OSCAT_BASIC_LIST: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_list.st"
));
const OSCAT_BASIC_BUFFER: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_buffer.st"
));
const OSCAT_BASIC_TIME: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../libraries/oscat_basic/src/oscat_basic_time.st"
));

const OSCAT_BASIC_GENERATOR_PROGRAM: &str = r#"
PROGRAM Main
VAR
    ClickInput : BOOL := FALSE;
    ClickResult : BOOL := FALSE;
    ClickDecode0 : BOOL := FALSE;
    ClickDecode1 : BOOL := FALSE;
    DividerClk : BOOL := FALSE;
    DividerReset : BOOL := FALSE;
    DividerBit0 : BOOL := FALSE;
    DividerBit1 : BOOL := FALSE;
    Counter : CLICK_CNT;
    Decoder : CLICK_DEC;
    Divider : CLK_DIV;
END_VAR

Counter(IN := ClickInput, N := INT#1, TC := T#50ms);
ClickResult := Counter.Q;

Decoder(IN := ClickInput, TC := T#50ms);
ClickDecode0 := Decoder.Q0;
ClickDecode1 := Decoder.Q1;

Divider(CLK := DividerClk, RST := DividerReset);
DividerBit0 := Divider.Q0;
DividerBit1 := Divider.Q1;
END_PROGRAM
"#;

const OSCAT_BASIC_CLOCK_PROGRAM: &str = r#"
PROGRAM Main
VAR
    PulseReset : BOOL := FALSE;
    SlowClock : CLK_N;
    PulseClock : CLK_PULSE;
    SquareClock : GEN_SQ;
    SlowPulse : BOOL := FALSE;
    PulseQ : BOOL := FALSE;
    PulseCount : INT := INT#0;
    PulseRun : BOOL := FALSE;
    SquareQ : BOOL := FALSE;
END_VAR

SlowClock(N := INT#1);
SlowPulse := SlowClock.Q;

PulseClock(PT := T#5ms, N := INT#2, RST := PulseReset);
PulseQ := PulseClock.Q;
PulseCount := PulseClock.CNT;
PulseRun := PulseClock.RUN;

SquareClock(PT := T#10ms);
SquareQ := SquareClock.Q;
END_PROGRAM
"#;

const OSCAT_BASIC_DELAY_PROGRAM: &str = r#"
PROGRAM Main
VAR
    DelayInput : BOOL := FALSE;
    TriggerInput : BOOL := FALSE;
    DelayFb : TONOF;
    PulseFb : TP_X;
    DelayQ : BOOL := FALSE;
    PulseQ : BOOL := FALSE;
    PulseEt : TIME := T#0ms;
END_VAR

DelayFb(IN := DelayInput, T_ON := T#5ms, T_OFF := T#5ms);
DelayQ := DelayFb.Q;

PulseFb(IN := TriggerInput, PT := T#10ms);
PulseQ := PulseFb.Q;
PulseEt := PulseFb.ET;
END_PROGRAM
"#;

const OSCAT_BASIC_SEQUENCE_PROGRAM: &str = r#"
PROGRAM Main
VAR
    Start4 : BOOL := FALSE;
    Start8 : BOOL := FALSE;
    Seq4 : SEQUENCE_4;
    Seq8 : SEQUENCE_8;
    Seq4Run : BOOL := FALSE;
    Seq4State : INT := INT#0;
    Seq4Q0 : BOOL := FALSE;
    Seq4Q1 : BOOL := FALSE;
    Seq4Qx : BOOL := FALSE;
    Seq8Run : BOOL := FALSE;
    Seq8State : INT := INT#0;
    Seq8Q0 : BOOL := FALSE;
    Seq8Q1 : BOOL := FALSE;
END_VAR

Seq4(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    START := Start4,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms
);
Seq4Run := Seq4.RUN;
Seq4State := Seq4.STATE;
Seq4Q0 := Seq4.Q0;
Seq4Q1 := Seq4.Q1;
Seq4Qx := Seq4.QX;

Seq8(
    IN0 := TRUE,
    IN1 := TRUE,
    IN2 := TRUE,
    IN3 := TRUE,
    IN4 := TRUE,
    IN5 := TRUE,
    IN6 := TRUE,
    IN7 := TRUE,
    START := Start8,
    RST := FALSE,
    WAIT0 := T#0ms,
    DELAY0 := T#0ms,
    WAIT1 := T#0ms,
    DELAY1 := T#0ms,
    WAIT2 := T#0ms,
    DELAY2 := T#0ms,
    WAIT3 := T#0ms,
    DELAY3 := T#0ms,
    WAIT4 := T#0ms,
    DELAY4 := T#0ms,
    WAIT5 := T#0ms,
    DELAY5 := T#0ms,
    WAIT6 := T#0ms,
    DELAY6 := T#0ms,
    WAIT7 := T#0ms,
    DELAY7 := T#0ms
);
Seq8Run := Seq8.RUN;
Seq8State := Seq8.STATE;
Seq8Q0 := Seq8.Q0;
Seq8Q1 := Seq8.Q1;
END_PROGRAM
"#;

fn harness(bytecode_vm: bool) -> TestHarness {
    let mut harness =
        TestHarness::from_sources(&oscat_basic_sources(OSCAT_BASIC_GENERATOR_PROGRAM))
            .expect("compile harness");
    if bytecode_vm {
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("select vm backend");
        harness
            .runtime_mut()
            .restart(trust_runtime::RestartMode::Cold)
            .expect("restart runtime");
    }
    harness
}

fn harness_from_program(source: &'static str, bytecode_vm: bool) -> TestHarness {
    let mut harness =
        TestHarness::from_sources(&oscat_basic_sources(source)).expect("compile harness");
    if bytecode_vm {
        harness
            .runtime_mut()
            .set_execution_backend(ExecutionBackend::BytecodeVm)
            .expect("select vm backend");
        harness
            .runtime_mut()
            .restart(trust_runtime::RestartMode::Cold)
            .expect("restart runtime");
    }
    harness
}

fn oscat_basic_sources(program: &'static str) -> [&'static str; 10] {
    // Generator FBs live in `oscat_basic_logic.st`, but that unit has cross-file
    // dependencies on the broader OSCAT BASIC surface. Compile these tests
    // against the real sibling units instead of ad hoc local shims.
    [
        OSCAT_BASIC_GLOBALS,
        OSCAT_BASIC_MATH,
        OSCAT_BASIC_MATH_EXTRA,
        OSCAT_BASIC_CONVERSIONS,
        OSCAT_BASIC_STRING,
        OSCAT_BASIC_LIST,
        OSCAT_BASIC_BUFFER,
        OSCAT_BASIC_TIME,
        OSCAT_BASIC_LOGIC,
        program,
    ]
}

fn assert_click_and_divider_behavior(harness: &mut TestHarness) {
    harness.set_input("DividerReset", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "reset cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DividerBit0", false);
    harness.assert_eq("DividerBit1", false);

    harness.set_input("DividerReset", false);
    harness.set_input("DividerClk", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "divider first cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DividerBit0", true);
    harness.assert_eq("DividerBit1", false);

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "divider second cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DividerBit0", false);
    harness.assert_eq("DividerBit1", true);

    harness.set_input("ClickInput", true);
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "click rise cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("ClickResult", false);
    harness.assert_eq("ClickDecode0", false);
    harness.assert_eq("ClickDecode1", false);

    harness.set_input("ClickInput", false);
    harness.advance_time(Duration::from_millis(10));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "click fall cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("ClickResult", false);
    harness.assert_eq("ClickDecode0", false);
    harness.assert_eq("ClickDecode1", false);

    harness.advance_time(Duration::from_millis(60));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "click expiry cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("ClickResult", false);
    harness.assert_eq("ClickDecode0", false);
    harness.assert_eq("ClickDecode1", false);

    harness.advance_time(Duration::from_millis(1));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "click report cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("ClickResult", true);
    harness.assert_eq("ClickDecode0", false);
    harness.assert_eq("ClickDecode1", true);

    harness.advance_time(Duration::from_millis(1));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "click clear cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("ClickResult", false);
    harness.assert_eq("ClickDecode0", false);
    harness.assert_eq("ClickDecode1", false);
}

fn assert_clock_behavior(harness: &mut TestHarness) {
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "init cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", false);
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseCount", 0i16);
    harness.assert_eq("PulseRun", false);
    harness.assert_eq("SquareQ", true);

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "first pulse cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", false);
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseCount", 1i16);
    harness.assert_eq("PulseRun", true);
    harness.assert_eq("SquareQ", true);

    harness.advance_time(Duration::from_millis(2));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "slow clock rise cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", true);
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseCount", 1i16);
    harness.assert_eq("PulseRun", true);
    harness.assert_eq("SquareQ", true);

    harness.advance_time(Duration::from_millis(3));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "second pulse cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", true);
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseCount", 2i16);
    harness.assert_eq("PulseRun", true);
    harness.assert_eq("SquareQ", false);

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse stop cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", false);
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseCount", 2i16);
    harness.assert_eq("PulseRun", false);
    harness.assert_eq("SquareQ", false);

    harness.set_input("PulseReset", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "reset cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseCount", 0i16);
    harness.assert_eq("PulseRun", false);

    harness.set_input("PulseReset", false);
    harness.advance_time(Duration::from_millis(5));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "post-reset pulse cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("SlowPulse", true);
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseCount", 1i16);
    harness.assert_eq("PulseRun", true);
    harness.assert_eq("SquareQ", true);
}

fn assert_delay_behavior(harness: &mut TestHarness) {
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "init cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DelayQ", false);
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(0)));

    harness.set_input("DelayInput", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "delay start cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DelayQ", false);

    harness.advance_time(Duration::from_millis(5));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "delay on cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DelayQ", true);

    harness.set_input("DelayInput", false);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "delay off start cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DelayQ", true);

    harness.advance_time(Duration::from_millis(5));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "delay off cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("DelayQ", false);

    harness.set_input("TriggerInput", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse start cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(0)));

    harness.set_input("TriggerInput", false);
    harness.advance_time(Duration::from_millis(5));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse mid cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(5)));

    harness.set_input("TriggerInput", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse retrigger cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(0)));

    harness.set_input("TriggerInput", false);
    harness.advance_time(Duration::from_millis(9));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse near-expiry cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", true);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(9)));

    harness.advance_time(Duration::from_millis(1));
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "pulse expiry cycle errors: {:?}",
        cycle.errors
    );
    harness.assert_eq("PulseQ", false);
    harness.assert_eq("PulseEt", Value::Time(Duration::from_millis(0)));
}

fn assert_sequence_behavior(harness: &mut TestHarness) {
    harness.set_input("Start4", true);
    harness.set_input("Start8", true);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "sequence start cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("Seq4Run"),
        Some(true.into()),
        "Seq4Run start"
    );
    assert_eq!(
        harness.get_output("Seq4State"),
        Some(0i16.into()),
        "Seq4State start"
    );
    assert_eq!(
        harness.get_output("Seq4Q0"),
        Some(true.into()),
        "Seq4Q0 start"
    );
    assert_eq!(
        harness.get_output("Seq4Q1"),
        Some(false.into()),
        "Seq4Q1 start"
    );
    assert_eq!(
        harness.get_output("Seq4Qx"),
        Some(true.into()),
        "Seq4Qx start"
    );
    assert_eq!(
        harness.get_output("Seq8Run"),
        Some(true.into()),
        "Seq8Run start"
    );
    assert_eq!(
        harness.get_output("Seq8State"),
        Some(0i16.into()),
        "Seq8State start"
    );
    assert_eq!(
        harness.get_output("Seq8Q0"),
        Some(true.into()),
        "Seq8Q0 start"
    );
    assert_eq!(
        harness.get_output("Seq8Q1"),
        Some(false.into()),
        "Seq8Q1 start"
    );

    harness.set_input("Start4", false);
    harness.set_input("Start8", false);
    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "sequence advance cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("Seq4State"),
        Some(1i16.into()),
        "Seq4State advance"
    );
    assert_eq!(
        harness.get_output("Seq4Q0"),
        Some(false.into()),
        "Seq4Q0 advance"
    );
    assert_eq!(
        harness.get_output("Seq4Q1"),
        Some(true.into()),
        "Seq4Q1 advance"
    );
    assert_eq!(
        harness.get_output("Seq8State"),
        Some(1i16.into()),
        "Seq8State advance"
    );
    assert_eq!(
        harness.get_output("Seq8Q0"),
        Some(false.into()),
        "Seq8Q0 advance"
    );
    assert_eq!(
        harness.get_output("Seq8Q1"),
        Some(true.into()),
        "Seq8Q1 advance"
    );

    let cycle = harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "sequence output cycle errors: {:?}",
        cycle.errors
    );
    assert_eq!(
        harness.get_output("Seq4Run"),
        Some(true.into()),
        "Seq4Run output"
    );
    assert_eq!(
        harness.get_output("Seq4State"),
        Some(2i16.into()),
        "Seq4State output"
    );
    assert_eq!(
        harness.get_output("Seq4Q0"),
        Some(false.into()),
        "Seq4Q0 output"
    );
    assert_eq!(
        harness.get_output("Seq4Q1"),
        Some(false.into()),
        "Seq4Q1 output"
    );
    assert_eq!(
        harness.get_output("Seq4Qx"),
        Some(true.into()),
        "Seq4Qx output"
    );
    assert_eq!(
        harness.get_output("Seq8Run"),
        Some(true.into()),
        "Seq8Run output"
    );
    assert_eq!(
        harness.get_output("Seq8State"),
        Some(2i16.into()),
        "Seq8State output"
    );
    assert_eq!(
        harness.get_output("Seq8Q0"),
        Some(false.into()),
        "Seq8Q0 output"
    );
    assert_eq!(
        harness.get_output("Seq8Q1"),
        Some(false.into()),
        "Seq8Q1 output"
    );
}

#[test]
fn oscat_basic_click_and_divider_generators_behave_in_runtime() {
    let mut harness = harness(false);
    assert_click_and_divider_behavior(&mut harness);
}

#[test]
fn oscat_basic_click_and_divider_generators_behave_in_vm() {
    let mut harness = harness(true);
    assert_click_and_divider_behavior(&mut harness);
}

#[test]
fn oscat_basic_clock_generators_behave_in_runtime() {
    let mut harness = harness_from_program(OSCAT_BASIC_CLOCK_PROGRAM, false);
    assert_clock_behavior(&mut harness);
}

#[test]
fn oscat_basic_clock_generators_behave_in_vm() {
    let mut harness = harness_from_program(OSCAT_BASIC_CLOCK_PROGRAM, true);
    assert_clock_behavior(&mut harness);
}

#[test]
fn oscat_basic_delay_generators_behave_in_runtime() {
    let mut harness = harness_from_program(OSCAT_BASIC_DELAY_PROGRAM, false);
    assert_delay_behavior(&mut harness);
}

#[test]
fn oscat_basic_delay_generators_behave_in_vm() {
    let mut harness = harness_from_program(OSCAT_BASIC_DELAY_PROGRAM, true);
    assert_delay_behavior(&mut harness);
}

#[test]
fn oscat_basic_sequence_generators_behave_in_runtime() {
    let mut harness = harness_from_program(OSCAT_BASIC_SEQUENCE_PROGRAM, false);
    assert_sequence_behavior(&mut harness);
}

#[test]
fn oscat_basic_sequence_generators_behave_in_vm() {
    let mut harness = harness_from_program(OSCAT_BASIC_SEQUENCE_PROGRAM, true);
    assert_sequence_behavior(&mut harness);
}
