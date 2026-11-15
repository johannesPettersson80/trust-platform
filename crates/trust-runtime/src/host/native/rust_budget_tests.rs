//! The admission counts a loop's passes and writes durations as RU-47's findings read them; the
//! findings of whole projects are proven by `tests/native_rust_pous.rs`.

use trust_syntax::parser::parse;
use trust_syntax::syntax::SyntaxKind;

use super::{duration, for_count, Times};

fn first_for(text: &str) -> trust_syntax::syntax::SyntaxNode {
    parse(text)
        .syntax()
        .descendants()
        .find(|node| node.kind() == SyntaxKind::ForStmt)
        .expect("a FOR loop")
}

#[test]
fn a_for_loop_with_constant_bounds_counts_its_passes() {
    // RU-47: a FOR loop with constant bounds makes (end - start) / step + 1 passes, none when it
    // does not run; another loop has no count. The body's statements stand in the loop itself, so
    // a statement taken for the step would leave every loop without a count. Catches that, a count
    // off by one, a step ignored and a based literal read as decimal.
    let body = |header: &str| {
        format!("PROGRAM P\nVAR\n    i : INT;\nEND_VAR\n{header} DO\n    i := i;\nEND_FOR;\nEND_PROGRAM\n")
    };
    for (header, count) in [
        ("FOR i := 1 TO 5", Some(5)),
        ("FOR i := 0 TO 9 BY 2", Some(5)),
        ("FOR i := 10 TO 1 BY -3", Some(4)),
        ("FOR i := 5 TO 1", Some(0)),
        ("FOR i := INT#1 TO 16#10", Some(16)),
        ("FOR i := INT#16#A TO 16#B", Some(2)),
        ("FOR i := 1 TO i", None),
        ("FOR i := 1 TO 5 BY 0", None),
    ] {
        assert_eq!(for_count(&first_for(&body(header))), count, "{header}");
    }
}

#[test]
fn a_loop_that_never_runs_makes_no_calls() {
    // RU-47: the calls inside a loop of no passes are never made, even inside a loop without a
    // bound there; a loop without a bound inside one that runs has no bound. Catches a loop that
    // never runs counted as unbounded.
    assert_eq!(Times::Count(0).times(Times::Unbounded), Times::Count(0));
    assert_eq!(Times::Unbounded.times(Times::Count(0)), Times::Count(0));
    assert_eq!(Times::Count(3).times(Times::Unbounded), Times::Unbounded);
    assert_eq!(Times::Count(3).times(Times::Count(4)), Times::Count(12));
}

#[test]
fn durations_read_in_the_unit_that_fits() {
    assert_eq!(duration(20_000_000), "20 ms");
    assert_eq!(duration(1_500_000), "1.5 ms");
    assert_eq!(duration(15_000), "15 µs");
    assert_eq!(duration(2_500), "2.5 µs");
    assert_eq!(duration(999), "999 ns");
}
