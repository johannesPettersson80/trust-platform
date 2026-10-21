//! The OC-1 bench program and the integer bodies on their operands' bits (checklist P1-9;
//! specification 34 PERF-05): kinded code, whose operands' kinds preparation fixes, ends each body
//! as the general path does.

use super::*;

/// The OC-1 bench program `v1p` in the six variables: a global, two standard blocks, a branch on an
/// output, integer arithmetic and conversions, and a TIME output converted to a DWORD, which compact
/// code runs on its operands' bits, their kinds fixed at preparation (checklist P1-9).
const BENCH_SOURCE: &str = "
PROGRAM Main
VAR_EXTERNAL
    runs : UDINT;
END_VAR
VAR
    count : UDINT := 100;
    double : DINT := 9;
    high : BOOL;
    level : UDINT;
    small : INT := 7;
    ticks : DWORD;
    clock : TON;
    edge : R_TRIG;
END_VAR
runs := runs + 1;
high := TRUE;
small := 1234;
double := 70000;
clock(IN := TRUE, PT := T#1h);
edge(CLK := TRUE);
IF edge.Q THEN
    count := count + 1;
END_IF;
level := count * 1000 + runs;
ticks := TIME_TO_DWORD(clock.ET);
double := UDINT_TO_DINT(count) * 10000 + INT_TO_DINT(small);
END_PROGRAM
CONFIGURATION Conf
VAR_GLOBAL
    runs : UDINT;
END_VAR
RESOURCE Controller ON PLC TASK Periodic (INTERVAL := T#10ms, PRIORITY := 0);
PROGRAM Plant WITH Periodic : Main; END_RESOURCE END_CONFIGURATION
";

/// Worked by hand from the initial values (count 100, double 9, small 7, the global `runs` 0) and
/// specification 08 §5: `runs` becomes 1; the timer's first call adds no elapsed time, so its `ET` is
/// T#0ms; the edge trigger's first call with CLK TRUE sets Q (its memory starts FALSE), so `count`
/// becomes 101; `level` is 101 × 1000 + 1 = 101001, `ticks` TIME_TO_DWORD(T#0ms) = 0 and `double`
/// 101 × 10000 + 1234 = 1011234 (specification 07 §2.1 for the conversions).
/// The integer bodies run on their operands' bits too: `TYPED_SOURCE`'s BOOL AND and OR, which the
/// compiler makes short-circuit, keep their left operand on the stack across their jump, and every
/// path to the jump's target brings a BOOL there.
/// Six of the program's statements are assignments of a constant, a variable or a chain of integer
/// operations on them, which kinded code runs as one operation each (spec 34 PERF-05): `runs :=
/// runs + 1`, `high := TRUE`, `small := 1234`, `double := 70000`, `count := count + 1` in the IF,
/// and `level := count * 1000 + runs`; the calls, the IF's condition and the conversions are not.
/// Every work allowance then refuses the run inside each of them at every instruction, so such a
/// statement must leave its charges, site and values exactly as its operations one by one leave
/// them.
/// Catches: kinded code that reads or writes a leaf with another width or sign, binds a block's
/// inputs or reads its outputs otherwise than the typed loop, or charges, records or faults
/// otherwise than the general path under any work allowance, a statement run at once that charges
/// another count of instructions, leaves another fault site or skips its refusal, and a
/// short-circuit AND or OR that keeps its body on the typed loop.
#[test]
fn the_bench_program_runs_on_its_operands_bits_as_on_the_general_path() {
    assert_eq!(
        compact_bodies(BENCH_SOURCE),
        1,
        "Main's body has compact code"
    );
    assert_eq!(
        kinded_bodies(BENCH_SOURCE),
        1,
        "every operand of Main's body has a kind fixed at preparation"
    );
    assert_eq!(
        fused_statements(BENCH_SOURCE),
        6,
        "six statements of Main run as one operation each"
    );
    assert_eq!(
        kinded_bodies(TYPED_SOURCE) + kinded_bodies(CONVERTING_SOURCE),
        2,
        "the integer bodies run on their operands' bits"
    );
    assert_eq!(
        kinded_bodies(TIMER_SOURCE),
        0,
        "duration arithmetic keeps a body on the typed loop"
    );
    every_work_allowance_ends_alike(BENCH_SOURCE);
    let (result, record, variables) = first_scan(&artifact(BENCH_SOURCE), 600, false);
    assert_eq!(result, Ok(()));
    assert_eq!(record, None);
    assert_eq!(
        variables,
        [
            Value::UDInt(101),
            Value::DInt(1_011_234),
            Value::Bool(true),
            Value::UDInt(101_001),
            Value::Int(1234),
            Value::DWord(0),
        ]
        .map(Some)
    );
}
