//! A POU's own scalar variables read and written in place (checklist P1-3 and P1-F; specification
//! 34 TIME-08 and MEM-05). When no statement hook observes the instructions, a body that has compact
//! code runs on the compact loop, and any other body's REF_FIELD runs with the LOAD after it, or
//! with the SWAP and STORE after it, as one. Either charges each instruction its unit after
//! recording it as the fault site, as the dispatcher does. Under every work allowance the first scan
//! therefore ends as it ends with a hook observing: with the same result, the same fault record and
//! the same variables. The tests catch an in-place access that charges instructions at once,
//! records the wrong instruction as the site, stores before a refused charge, or reads or writes
//! another variable, a conversion that converts or charges otherwise than its call, a function
//! block's output read from another place or charged otherwise than its four instructions, and a
//! program that loses its compact code.

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::fault::FaultRecord;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

/// Own variables of every scalar kind the program reads and writes, in arithmetic, a comparison and
/// a branch.
const SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : REAL := 1.5;
    small : INT := 7;
    ticks : TIME := T#2s;
END_VAR
count := count + 1;
double := count * 2;
high := double > 3;
level := level * 2.0;
IF high THEN
    count := count + double;
END_IF;
small := small - 1;
ticks := ticks + T#1s;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// The same variables with a standard function call, which compact code runs through the
/// dispatcher's own call (checklist P1-F), and a standard conversion, which it runs where the call
/// is (checklist P1-5).
const CALLING_SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : REAL := 1.5;
    small : INT := 7;
    ticks : TIME := T#2s;
END_VAR
count := count + 1;
double := ABS(count * 2);
high := double > 3;
level := level * DINT_TO_REAL(count);
IF high THEN
    count := count + double;
END_IF;
small := small - 1;
ticks := ticks + T#1s;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// The same variables with a timer whose outputs the body reads, also as a conversion's argument,
/// which compact code reads in their leaves (checklist P1-F, P1-5). A zero preset completes on the
/// first call with IN TRUE (spec 08 §5), so the scan reads Q TRUE.
const MEMBER_SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : REAL := 1.5;
    small : INT := 7;
    ticks : TIME := T#2s;
    delay : TON;
END_VAR
count := count + 1;
delay(IN := count > 1, PT := T#0ms);
double := count * 2;
high := delay.Q;
level := level * 2.0;
IF high THEN
    count := count + double;
END_IF;
small := small - BOOL_TO_INT(delay.Q);
ticks := delay.ET + T#1s;
END_PROGRAM

CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// The same variable names with two timers: an overloaded `TON`, whose `PT` and `ET` are native
/// duration slots (specification 34 §6.2), bound with a preset and read as a conversion's argument
/// and in arithmetic, and a `TON_LTIME`, whose `ET` is an `LTIME` (specification 08, timer
/// variants). Compact code writes the presets and reads the elapsed times in their leaves
/// (checklist P1-5).
const TIMER_SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DWORD;
    high : BOOL;
    level : LTIME := LTIME#1s;
    small : INT := 7;
    ticks : TIME := T#2s;
    delay : TON;
    slow : TON_LTIME;
END_VAR
count := count + 1;
delay(IN := count > 1, PT := T#0ms);
slow(IN := TRUE, PT := LTIME#5ms);
double := TIME_TO_DWORD(delay.ET);
high := delay.Q AND NOT slow.Q;
level := level + slow.ET;
small := small - BOOL_TO_INT(delay.Q);
ticks := delay.ET + T#1s;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// The same variables with a call of a user function, an instruction compact code has no operation
/// for, so the body runs on the general path and fuses its own variables' accesses (checklist P1-3).
const FUSED_SOURCE: &str = "
FUNCTION Twice : DINT
VAR_INPUT
    x : DINT;
END_VAR
Twice := x * 2;
END_FUNCTION

PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : REAL := 1.5;
    small : INT := 7;
    ticks : TIME := T#2s;
END_VAR
count := count + 1;
double := Twice(count);
high := double > 3;
level := level * 2.0;
IF high THEN
    count := count + double;
END_IF;
small := small - 1;
ticks := ticks + T#1s;
END_PROGRAM

CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

const VARIABLES: [&str; 6] = ["count", "double", "high", "level", "small", "ticks"];

/// Services that keep the stack path; `observe` says whether the statement hook observes.
struct Stack {
    observe: bool,
}

impl ExecutionServices for Stack {
    fn debug_active(&self) -> bool {
        true
    }
    fn observes_statements(&self) -> bool {
        self.observe
    }
    fn deadline_exceeded(&self) -> bool {
        false
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

/// How the first scan ended: its result, the fault record it latched and the program's variables.
type Outcome = (
    Result<(), RuntimeError>,
    Option<FaultRecord>,
    Vec<Option<Value>>,
);

fn first_scan(bytes: &[u8], max_work: usize, observe: bool) -> Outcome {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(bytes, limits).expect("the program is admitted");
    let services = Stack { observe };
    let mut state = prepared
        .instantiate_with_services(0, &services)
        .expect("construction has its own allowance");
    let result = state.execute_cycle(Duration::from_millis(10));
    let record = state.fault_record();
    let Some(Value::Instance(plant)) = state.storage().get_global("Plant").cloned() else {
        panic!("the program instance is missing");
    };
    let variables = VARIABLES
        .iter()
        .map(|name| state.storage().get_instance_var(plant, name).cloned())
        .collect();
    (result, record, variables)
}

fn artifact(source: &str) -> Vec<u8> {
    CompileSession::from_source(source)
        .build_bytecode_module_for_version(BytecodeVersion::SOURCE_FREE)
        .expect("the program compiles")
        .encode()
        .expect("the program encodes")
}

fn compact_bodies(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .compact_bodies()
}

fn compact_block_calls(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .compact_block_calls()
}

#[test]
fn a_body_of_own_scalars_arithmetic_and_branches_has_compact_code() {
    assert_eq!(compact_bodies(SOURCE), 1, "Main's body has compact code");
    assert_eq!(
        compact_bodies(CALLING_SOURCE),
        1,
        "a standard function call keeps Main's compact code"
    );
    assert_eq!(
        compact_bodies(MEMBER_SOURCE),
        1,
        "a timer's outputs read in the body keep Main's compact code"
    );
    assert_eq!(
        compact_block_calls(MEMBER_SOURCE),
        1,
        "the timer's call binds input values only, so compact code runs it where it lies"
    );
    assert_eq!(
        compact_bodies(FUSED_SOURCE),
        0,
        "a user function call keeps Main on the general path"
    );
}

#[test]
fn every_work_allowance_ends_a_compact_first_scan_as_with_an_observing_hook() {
    every_work_allowance_ends_alike(SOURCE);
}

#[test]
fn every_work_allowance_ends_a_compact_calling_first_scan_as_with_an_observing_hook() {
    every_work_allowance_ends_alike(CALLING_SOURCE);
}

#[test]
fn every_work_allowance_ends_a_compact_member_reading_first_scan_as_with_an_observing_hook() {
    every_work_allowance_ends_alike(MEMBER_SOURCE);
}

#[test]
fn every_work_allowance_ends_a_fused_first_scan_as_with_an_observing_hook() {
    every_work_allowance_ends_alike(FUSED_SOURCE);
}

/// Worked out from specification 08's timer contract for the first scan: `count` becomes 2, so
/// `delay` runs with `IN` TRUE and a zero preset, which completes on that call (`Q` TRUE, `ET` 0 ms);
/// `slow`'s first call adds no elapsed time (`Q` FALSE, `ET` 0). So `double` is
/// `TIME_TO_DWORD(T#0ms)` = 0, `high` is TRUE, `level` stays 1 s, `small` is 7 - 1 = 6 and `ticks`
/// is 0 ms + 1 s.
/// Catches: a preset written into a duration slot in another form than the call binds it (the
/// timer would not complete), an elapsed time read from the wrong bytes or with the wrong family,
/// and a conversion of a timer's output that converts otherwise than its call.
#[test]
fn a_timers_duration_members_are_written_and_read_in_place() {
    assert_eq!(
        compact_bodies(TIMER_SOURCE),
        1,
        "two timers and a conversion of an elapsed time keep Main's compact code"
    );
    assert_eq!(
        compact_block_calls(TIMER_SOURCE),
        2,
        "both timer calls bind input values only, a duration preset included"
    );
    every_work_allowance_ends_alike(TIMER_SOURCE);
    let (result, record, variables) = first_scan(&artifact(TIMER_SOURCE), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::DInt(2),
            Value::DWord(0),
            Value::Bool(true),
            Value::LTime(Duration::from_secs(1)),
            Value::Int(6),
            Value::Time(Duration::from_secs(1)),
        ]
        .map(Some)
    );
}

fn every_work_allowance_ends_alike(source: &str) {
    let bytes = artifact(source);
    let mut admitted = None;
    for max_work in 1..=600 {
        let observed = first_scan(&bytes, max_work, true);
        let in_place = first_scan(&bytes, max_work, false);
        assert_eq!(
            observed, in_place,
            "work allowance {max_work}: with an observing hook {observed:?}, in place {in_place:?}"
        );
        if observed.0.is_ok() {
            admitted = Some(max_work);
            break;
        }
    }
    let admitted = admitted.expect("an allowance of 600 units admits the first scan");
    // The smallest allowance that admits the scan is its work: 1 unit for each instruction the
    // scan runs, so the refusals above cover every instruction it reaches.
    assert!(admitted > 40, "the scan ran {admitted} units");
}
