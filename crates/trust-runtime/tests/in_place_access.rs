//! A POU's own scalar variables read and written in place (checklist P1-3, P1-F and P1-9;
//! specification 34 TIME-08, MEM-05 and PERF-05). When no statement hook observes the instructions,
//! a body that has compact code runs on the typed loop, which holds its scalar operands as cells and
//! hands the rest of the body to the compact loop at an operand it does not hold, and any other
//! body's REF_FIELD runs with the LOAD after it, or with the SWAP and STORE after it, as one. Each
//! charges each instruction its unit after recording it as the fault site, as the dispatcher does.
//! Under every work allowance the first scan therefore ends as it ends with a hook observing: with
//! the same result, the same fault record and the same variables. The tests catch an in-place
//! access that charges instructions at once, records the wrong instruction as the site, stores
//! before a refused charge, or reads or writes another variable, a conversion that converts or
//! charges otherwise than its call, a function block's output read from another place or charged
//! otherwise than its four instructions, integer arithmetic on cells that computes or refuses
//! otherwise than on values, a hand-over that loses, reorders or repeats an operand or an
//! instruction, and a program that loses its compact code.

use trust_runtime::bytecode::BytecodeVersion;
use trust_runtime::harness::CompileSession;
use trust_runtime_core::error::RuntimeError;
use trust_runtime_core::fault::FaultRecord;
use trust_runtime_core::value::{DateTimeValue, Duration, Value};
use trust_runtime_core::vm::{ExecutionServices, PreparationLimits, PreparedModule};

#[path = "in_place_access/kinded.rs"]
mod kinded;

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

/// Own integers of every width and signedness in arithmetic that widens, divides and takes a
/// remainder, comparisons, BOOL logic, a branch, and a store of a USINT into a UDINT, which the
/// general store widens (checklist P1-9).
const TYPED_SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 7;
    double : UDINT;
    high : BOOL;
    level : LINT := -3;
    small : SINT := -100;
    ticks : USINT := 200;
END_VAR
count := count * 3 - count / 2 + count MOD 4 + small;
double := ticks;
double := double * 2 + ticks - double / 3;
level := level * count + small;
high := count > 20 AND NOT (ticks = 200) OR level < 0 XOR small <= -100;
IF count >= 21 AND high THEN
    ticks := ticks + 1;
ELSE
    ticks := ticks - 1;
END_IF;
small := small - 27;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// Integer conversions of own variables across widths and signedness, each naming its source type,
/// which compact code converts by the conversion's integer branch (checklist P1-9).
const CONVERTING_SOURCE: &str = "
PROGRAM Main
VAR
    count : DINT := 70000;
    double : UDINT := 4000000000;
    high : BOOL;
    level : LINT;
    small : SINT := -100;
    ticks : USINT := 200;
END_VAR
level := DINT_TO_LINT(count) + UDINT_TO_LINT(double);
count := SINT_TO_DINT(small) * 2;
ticks := DINT_TO_USINT(count + 255);
high := UDINT_TO_ULINT(double) > ULINT#3000000000;
small := USINT_TO_SINT(ticks) - 1;
double := LINT_TO_UDINT(level - LINT#4000000000) + DINT_TO_UDINT(count + 300);
level := level - SINT_TO_LINT(small) - USINT_TO_LINT(ticks);
count := count - USINT_TO_DINT(ticks) + SINT_TO_DINT(small);
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// A structure's field read in the middle of an expression: the typed loop holds `count` when it
/// meets the reference to `pair`, and the compact loop runs the rest of the body (checklist P1-9).
const STRUCT_SOURCE: &str = "
TYPE Pair : STRUCT x : DINT; y : DINT; END_STRUCT; END_TYPE

PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : LINT;
    small : SINT := 5;
    ticks : USINT := 7;
    pair : Pair := (x := 5, y := 6);
END_VAR
double := count + pair.x * 2;
count := count + 1;
small := small * 2;
high := double > count;
ticks := ticks + 1;
level := level + 1;
level := level + count * 3;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// A program whose first statement nests `depth` additions to the right, so that `depth + 1`
/// operands wait at once, followed by statements the compact loop runs when the typed loop has
/// handed the body over at its stack's room (checklist P1-9).
fn deep_source(depth: usize) -> String {
    let mut sum = String::from("1");
    for _ in 0..depth {
        sum = format!("count + ({sum})");
    }
    format!(
        "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : LINT;
    small : SINT := 5;
    ticks : USINT;
END_VAR
double := {sum};
small := small + 1;
high := double = 19;
ticks := 3;
level := double * 2;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
"
    )
}

/// A program of `body` over the six variables, each with its initial value: count 1, double 0,
/// high FALSE, level 9, small 120 and ticks 0.
fn faulting_source(body: &str) -> String {
    format!(
        "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
    level : LINT := 9;
    small : SINT := 120;
    ticks : USINT;
END_VAR
{body}
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
"
    )
}

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

/// Services that keep the stack path, whose deadline passes at its sample after the first `after`
/// and stays passed; `observe` says whether the statement hook observes.
struct Deadline {
    observe: bool,
    after: usize,
    samples: std::cell::Cell<usize>,
}

impl ExecutionServices for Deadline {
    fn debug_active(&self) -> bool {
        true
    }
    fn observes_statements(&self) -> bool {
        self.observe
    }
    fn deadline_exceeded(&self) -> bool {
        let samples = self.samples.get() + 1;
        self.samples.set(samples);
        samples > self.after
    }
    fn has_wall_clock(&self) -> bool {
        false
    }
    fn current_dt(&self) -> Result<DateTimeValue, RuntimeError> {
        Err(RuntimeError::UndefinedFunction("CURRENT_DT".into()))
    }
}

fn first_scan(bytes: &[u8], max_work: usize, observe: bool) -> Outcome {
    scan(bytes, max_work, &Stack { observe })
}

/// The first scan of the program `bytes` under a work allowance of `max_work` with `services`.
fn scan(bytes: &[u8], max_work: usize, services: &impl ExecutionServices) -> Outcome {
    let limits = PreparationLimits {
        max_work,
        ..Default::default()
    };
    let prepared = PreparedModule::from_bytes(bytes, limits).expect("the program is admitted");
    // Construction has its own work allowance, but samples the deadline.
    let mut state = match prepared.instantiate_with_services(0, services) {
        Ok(state) => state,
        Err(error) => return (Err(error), None, Vec::new()),
    };
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

fn frameless_bodies(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .frameless_bodies()
}

fn compact_block_calls(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .compact_block_calls()
}

fn kinded_bodies(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .kinded_bodies()
}

fn block_member_places(source: &str) -> usize {
    PreparedModule::from_bytes(&artifact(source), PreparationLimits::default())
        .expect("the program is admitted")
        .block_member_places()
}

#[test]
fn a_body_of_own_scalars_arithmetic_and_branches_has_compact_code() {
    assert_eq!(compact_bodies(SOURCE), 1, "Main's body has compact code");
    // Specification 34 PERF-05: a program's body that keeps no variable in a frame starts without
    // one (checklist P1-9); every source here is such a body, so the equivalence tests below run
    // the frameless entry, the structure's and the call's pushing the frame when they need it.
    for source in [
        SOURCE,
        CALLING_SOURCE,
        MEMBER_SOURCE,
        TIMER_SOURCE,
        TYPED_SOURCE,
        STRUCT_SOURCE,
    ] {
        assert_eq!(frameless_bodies(source), 1, "Main starts without a frame");
    }
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
    // Specification 34 PERF-05 (checklist P1-4): TON and TON_LTIME each declare IN, PT, Q, ET
    // (IEC 61131-3 Table 43) and the slot of the time of their last call, and the native code reads
    // and writes all five where they lie in the instance.
    assert_eq!(
        block_member_places(TIMER_SOURCE),
        10,
        "each timer's five members have their places"
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

/// Specification 10 §6.2 to §6.4, worked by hand for the first scan: `count` is 7 × 3 − 7 / 2 +
/// 7 MOD 4 + (−100) = 21 − 3 + 3 − 100 = −79 in DINT, SINT widened; `double` takes `ticks`, 200, then
/// 200 × 2 + 200 − 200 / 3 = 400 + 200 − 66 = 534 in UDINT; `level` is −3 × −79 + (−100) = 137 in
/// LINT; `high` is (−79 > 20 AND NOT (200 = 200)) OR (137 < 0 XOR −100 <= −100) = FALSE OR (FALSE
/// XOR TRUE) = TRUE, AND before XOR before OR; −79 >= 21 is FALSE, so the ELSE branch makes `ticks`
/// 199; `small` is −100 − 27 = −127.
/// Catches: a typed operation that narrows, truncates or orders otherwise than specification 10, a
/// widened store written as the narrow value, a branch taken on the wrong cell, and charges, sites
/// or refusals that differ from the general path under any work allowance.
#[test]
fn integer_and_bool_operands_are_computed_on_their_cells() {
    assert_eq!(
        compact_bodies(TYPED_SOURCE),
        1,
        "Main's body has compact code"
    );
    every_work_allowance_ends_alike(TYPED_SOURCE);
    let (result, record, variables) = first_scan(&artifact(TYPED_SOURCE), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::DInt(-79),
            Value::UDInt(534),
            Value::Bool(true),
            Value::LInt(137),
            Value::SInt(-127),
            Value::USInt(199),
        ]
        .map(Some)
    );
}

/// Worked by hand from the initial values (count 70000, double 4000000000, small −100, ticks 200)
/// with specification 07 §2.1, by which a representable integer keeps its value exactly: `level`
/// 70000 + 4000000000 = 4000070000, `count` −100 × 2 = −200, `ticks` −200 + 255 = 55, `high`
/// 4000000000 > 3000000000 = TRUE, `small` 55 − 1 = 54, `double` 70000 + 100 = 70100, `level`
/// 4000070000 − 54 − 55 = 4000069891 and `count` −200 − 55 + 54 = −201.
/// Catches: an integer conversion of compact code that wraps, loses the sign or converts to
/// another width, or charges or records otherwise than the general path under any work allowance.
#[test]
fn integer_conversions_convert_as_on_the_general_path() {
    assert_eq!(
        compact_bodies(CONVERTING_SOURCE),
        1,
        "Main's body has compact code"
    );
    every_work_allowance_ends_alike(CONVERTING_SOURCE);
    let (result, record, variables) = first_scan(&artifact(CONVERTING_SOURCE), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::DInt(-201),
            Value::UDInt(70_100),
            Value::Bool(true),
            Value::LInt(4_000_069_891),
            Value::SInt(54),
            Value::USInt(55),
        ]
        .map(Some)
    );
}

/// Worked by hand: `double` is 1 + 5 × 2 = 11, then `count` 2, `small` 10, `high` 11 > 2 = TRUE,
/// `ticks` 8 and `level` 0 + 1 + 2 × 3 = 7. The typed loop hands the body over at the reference to
/// `pair` with `count` held, and the compact loop computes the rest.
/// Catches: a hand-over that drops, repeats or reorders the operand held, charges or records an
/// instruction twice or not at all, or restarts the body.
#[test]
fn a_reference_hands_the_body_over_with_its_operands() {
    assert_eq!(
        compact_bodies(STRUCT_SOURCE),
        1,
        "Main's body has compact code"
    );
    every_work_allowance_ends_alike(STRUCT_SOURCE);
    let (result, record, variables) = first_scan(&artifact(STRUCT_SOURCE), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::DInt(2),
            Value::DInt(11),
            Value::Bool(true),
            Value::LInt(7),
            Value::SInt(10),
            Value::USInt(8),
        ]
        .map(Some)
    );
}

/// Worked by hand: 18 additions of `count`, 1, to 1 make `double` 19, with 19 operands waiting at
/// the deepest, more than the typed loop's 16; then `small` is 6, `high` TRUE, `ticks` 3 and `level`
/// 19 × 2 = 38, the DINT widened to LINT by the general store. Depths of 15 and 16 bracket the
/// room: 15 additions make 16 with 16 operands at the deepest, the loop's whole room, and 16 make 17
/// with 17, one more than it holds.
/// Catches: a push past the typed loop's room, a hand-over at the room that loses an operand, and an
/// operand stack whose limit is met at another instruction.
#[test]
fn a_stack_deeper_than_the_typed_loops_room_hands_the_body_over() {
    let source = deep_source(18);
    assert_eq!(compact_bodies(&source), 1, "Main's body has compact code");
    every_work_allowance_ends_alike(&source);
    let (result, record, variables) = first_scan(&artifact(&source), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::DInt(1),
            Value::DInt(19),
            Value::Bool(true),
            Value::LInt(38),
            Value::SInt(6),
            Value::USInt(3),
        ]
        .map(Some)
    );
    for depth in [15, 16] {
        let source = deep_source(depth);
        every_work_allowance_ends_alike(&source);
        let (result, _, variables) = first_scan(&artifact(&source), 600, false);
        assert_eq!(result, Ok(()));
        assert_eq!(
            variables[1],
            Some(Value::DInt(i32::try_from(depth + 1).unwrap())),
            "depth {depth}"
        );
    }
}

/// Specification 10 §6.4, worked by hand from the initial values (count 1, level 9, small 120,
/// ticks 0): SINT 120 + 10 = 130 is above 127; `count` 2 / (2 − 2) divides by zero; LINT 10 MOD
/// (10 − 10) takes a remainder by zero; USINT 2 − 3 is below 0; DINT_TO_SINT(300) is above 127
/// (specification 07 §2.1). Each fault ends the scan before the faulting assignment stores, so the
/// statements before it have run and the rest have not.
/// Catches: a typed operation that wraps or saturates instead of refusing, refuses with another
/// error, stores the faulting result, or records another fault site or charges otherwise than the
/// general path under any work allowance.
#[test]
fn integer_faults_end_the_scan_as_on_the_general_path() {
    let cases: [(&str, RuntimeError, [Value; 6]); 5] = [
        (
            "count := count + 1; small := small + 10; count := count + 100;",
            RuntimeError::Overflow,
            [
                Value::DInt(2),
                Value::DInt(0),
                Value::Bool(false),
                Value::LInt(9),
                Value::SInt(120),
                Value::USInt(0),
            ],
        ),
        (
            "count := count + 1; double := count / (count - 2); count := 50;",
            RuntimeError::DivisionByZero,
            [
                Value::DInt(2),
                Value::DInt(0),
                Value::Bool(false),
                Value::LInt(9),
                Value::SInt(120),
                Value::USInt(0),
            ],
        ),
        (
            "level := level + 1; level := level MOD (level - 10); high := TRUE;",
            RuntimeError::ModuloByZero,
            [
                Value::DInt(1),
                Value::DInt(0),
                Value::Bool(false),
                Value::LInt(10),
                Value::SInt(120),
                Value::USInt(0),
            ],
        ),
        (
            "ticks := ticks + 2; ticks := ticks - 3; high := TRUE;",
            RuntimeError::Overflow,
            [
                Value::DInt(1),
                Value::DInt(0),
                Value::Bool(false),
                Value::LInt(9),
                Value::SInt(120),
                Value::USInt(2),
            ],
        ),
        (
            "count := count + 299; small := DINT_TO_SINT(count); high := TRUE;",
            RuntimeError::Overflow,
            [
                Value::DInt(300),
                Value::DInt(0),
                Value::Bool(false),
                Value::LInt(9),
                Value::SInt(120),
                Value::USInt(0),
            ],
        ),
    ];
    for (body, error, expected) in cases {
        let source = faulting_source(body);
        assert_eq!(
            compact_bodies(&source),
            1,
            "{body}: Main's body has compact code"
        );
        let (result, record, variables) = every_work_allowance_faults_alike(&source, &error);
        assert_eq!(result, Err(error), "{body}");
        assert!(record.is_some(), "{body}: the fault is recorded");
        assert_eq!(variables, expected.map(Some), "{body}");
    }
}

/// Specification 34 TIME-08 and PERF-05: the deadline is sampled every 32 units of work, at the
/// same instructions on every path, and the instructions of one operation are charged at once only
/// when none of their charges would sample it (checklist P1-9). For each sample at which the
/// deadline passes, the first scan ends as with an observing hook: with the same timeout, at the
/// same instruction, with the same variables.
/// Catches: a run of charges that skips a deadline sample, so that the scan stops at a later
/// instruction or not at all, and a sample taken at another instruction.
#[test]
fn every_deadline_sample_ends_a_compact_first_scan_as_with_an_observing_hook() {
    let deep = deep_source(18);
    for source in [
        SOURCE,
        MEMBER_SOURCE,
        TIMER_SOURCE,
        TYPED_SOURCE,
        CONVERTING_SOURCE,
        STRUCT_SOURCE,
        deep.as_str(),
    ] {
        let bytes = artifact(source);
        let mut completed = false;
        for after in 0..200 {
            let deadline = |observe| Deadline {
                observe,
                after,
                samples: std::cell::Cell::new(0),
            };
            let observed = scan(&bytes, 600, &deadline(true));
            let in_place = scan(&bytes, 600, &deadline(false));
            assert_eq!(
                observed, in_place,
                "deadline after {after} samples: with an observing hook {observed:?}, in place \
                 {in_place:?}"
            );
            if in_place.0.is_ok() {
                completed = true;
                break;
            }
        }
        assert!(
            completed,
            "the scan completes once the deadline outlasts its samples"
        );
    }
}

/// Services that observe statements and record the code offset of each call of the hook;
/// `starts_only` says that their hook acts only at statement starts.
struct Recording {
    starts_only: bool,
    calls: std::cell::RefCell<Vec<usize>>,
}

impl ExecutionServices for Recording {
    fn debug_active(&self) -> bool {
        true
    }
    fn observes_statements(&self) -> bool {
        true
    }
    fn hook_acts_at_statement_starts(&self) -> bool {
        self.starts_only
    }
    fn on_statement(
        &self,
        _pou_id: u32,
        code_offset: usize,
        _depth: u32,
        _storage: &trust_runtime_core::memory::VariableStorage,
        _logical_time: Duration,
    ) {
        self.calls.borrow_mut().push(code_offset);
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

/// Specification 13, "Safe Points", and specification 34 PERF-05 (checklist P1-10): a hook that
/// acts only at statement starts is called only at the first instruction of each statement the
/// `LINE_TABLE` names. Worked by hand: Main's body is three assignments, three statements, so the
/// first scan calls such a hook three times; services whose hook acts anywhere are called at every
/// instruction the scan runs, more than three, the three starts among them in the same order.
/// Catches: a hook skipped at a statement start, where a breakpoint would no longer stop, a hook
/// still called at every instruction, and calls at instructions that start no statement.
#[test]
fn a_hook_that_acts_at_statement_starts_is_called_only_there() {
    let source = "
PROGRAM Main
VAR
    count : DINT := 1;
    double : DINT;
    high : BOOL;
END_VAR
count := count + 1;
double := count * 2;
high := double > 3;
END_PROGRAM
CONFIGURATION Conf RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";
    let bytes = artifact(source);
    let prepared = PreparedModule::from_bytes(&bytes, PreparationLimits::default())
        .expect("the program is admitted");
    assert!(
        prepared.has_statement_index(),
        "the compiler writes a LINE_TABLE"
    );
    let calls = |starts_only| {
        let services = Recording {
            starts_only,
            calls: std::cell::RefCell::new(Vec::new()),
        };
        let (result, _, _) = scan(&bytes, 600, &services);
        assert_eq!(result, Ok(()));
        services.calls.into_inner()
    };
    let starts = calls(true);
    let every = calls(false);
    assert_eq!(starts.len(), 3, "one call per statement: {starts:?}");
    assert!(every.len() > 3, "a call per instruction: {every:?}");
    let starts_in_every: Vec<usize> = every
        .iter()
        .copied()
        .filter(|offset| starts.contains(offset))
        .collect();
    assert_eq!(starts_in_every, starts);
}

/// Under every work allowance the first scan of `source` ends as with an observing hook, until an
/// allowance covers the scan's instructions up to its fault, `error`; that scan's outcome.
fn every_work_allowance_faults_alike(source: &str, error: &RuntimeError) -> Outcome {
    let bytes = artifact(source);
    for max_work in 1..=600 {
        let observed = first_scan(&bytes, max_work, true);
        let in_place = first_scan(&bytes, max_work, false);
        assert_eq!(
            observed, in_place,
            "work allowance {max_work}: with an observing hook {observed:?}, in place {in_place:?}"
        );
        if in_place.0.as_ref().err() == Some(error) {
            return in_place;
        }
    }
    panic!("an allowance of 600 units reaches the fault {error:?}");
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
