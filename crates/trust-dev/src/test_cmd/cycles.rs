// Multi-cycle tests (specification 22, "Multi-cycle tests"): a test that runs its body once per cycle, in simulated time,
// until it calls `TEST_FINISHED()`, within its budget. One loop serves this computer and a device; each gives it a step that
// runs one cycle.

/// What one cycle of a multi-cycle test gave.
enum CycleResult {
    /// The cycle ran to its end; `true` once the test has called `TEST_FINISHED()`.
    Ran(bool),
    /// An assertion failed, with its message.
    Failed(String),
    /// Another error ended the cycle, with its text.
    Error(String),
}

/// The cycles a multi-cycle test ran and the program's time in the last, which its report carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CyclesRun {
    cycles: u32,
    time_ns: u64,
}

/// The program's time `time_ns` as the runtime writes a duration, in its largest whole unit (`T#200ms`, `T#1250ms`).
fn time_text(time_ns: u64) -> String {
    use trust_runtime::value::{format_user_value, Duration, Value};
    format_user_value(&Value::Time(Duration::from_nanos(
        i64::try_from(time_ns).unwrap_or(i64::MAX),
    )))
}

/// Runs a multi-cycle test with `step`, which runs cycle `n` (counted from 1) at the program's time `(n − 1)·cycle` and says how
/// it ended. The test passes in the cycle that calls `TEST_FINISHED()`, unless an assertion failed; a failed assertion fails it
/// and another error ends it as an error, each message naming the cycle and the time; a test that has not finished when its
/// budget runs out fails. The per-case deadline of `timeout_seconds`, counted from `started`, bounds the whole test in wall-clock
/// time.
fn run_cycles(
    cycles: TestCycles,
    timeout_seconds: u64,
    started: Instant,
    mut step: impl FnMut(u32, u64) -> CycleResult,
) -> (TestOutcome, Option<String>, CyclesRun) {
    let deadline = std::time::Duration::from_secs(timeout_seconds);
    let mut run = CyclesRun {
        cycles: 0,
        time_ns: 0,
    };
    for n in 1..=cycles.max_cycles {
        if started.elapsed() >= deadline {
            return (TestOutcome::Error, Some(timeout_message(timeout_seconds)), run);
        }
        let time_ns = u64::from(n - 1).saturating_mul(cycles.cycle_ns);
        run = CyclesRun { cycles: n, time_ns };
        match step(n, time_ns) {
            CycleResult::Ran(true) => return (TestOutcome::Passed, None, run),
            CycleResult::Ran(false) => {}
            CycleResult::Failed(message) => {
                let at = time_text(time_ns);
                return (
                    TestOutcome::Failed,
                    Some(format!("{message} (cycle {n}, t = {at})")),
                    run,
                );
            }
            CycleResult::Error(text) => {
                let at = time_text(time_ns);
                return (
                    TestOutcome::Error,
                    Some(format!("{text} (cycle {n}, t = {at})")),
                    run,
                );
            }
        }
    }
    let message = format!(
        "did not finish within {} cycles (t = {})",
        cycles.max_cycles,
        time_text(run.time_ns)
    );
    (TestOutcome::Failed, Some(message), run)
}
