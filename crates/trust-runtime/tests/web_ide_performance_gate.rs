//! The web IDE's reference performance gates (specification 11, 6.8.1e), in a test program of their
//! own so that they measure the web IDE alone. Inside the web IDE's integration tests they also
//! measured the tests running beside them, and on CI's runners they crossed their budgets at random:
//! the 800 ms completion maximum (28 October 2026) and the 250 ms maximum of an HTTP apply, which took
//! 323 ms (5 November 2026).
#![allow(dead_code, unused_imports)]

include!("web_ide_integration/helpers.rs");

/// Held by each gate while it measures, so that under `cargo test` the gates take turns and neither
/// measures the other; CI's cargo-nextest profile runs each gate alone (`.config/nextest.toml`).
static MEASURING: Mutex<()> = Mutex::new(());

/// Whether this run is the reference environment that asserts the budgets: an optimized build on
/// Linux x86_64 (specification 11, 6.8.1e). Every other run records its measurements only.
const REFERENCE_ENVIRONMENT: bool = cfg!(all(
    not(debug_assertions),
    target_os = "linux",
    target_arch = "x86_64"
));

/// Records `measured` beside its `budget` in the test log and, on the reference environment, fails
/// when `within`, the specification's comparison of the two, does not hold.
fn check_budget(what: &str, within: bool, measured: Duration, budget: Duration) {
    eprintln!(
        "web IDE budget: {what} {measured:?}, budget {budget:?}{}",
        if REFERENCE_ENVIRONMENT {
            ""
        } else {
            " (recorded, not asserted: not the reference environment)"
        }
    );
    assert!(
        within || !REFERENCE_ENVIRONMENT,
        "{what} exceeded its {budget:?} budget: {measured:?}"
    );
}

/// Specification 11, 6.8.1e: "one source apply must remain below 250 ms, and the repeated local
/// in-process average below 40 ms", measured on the IDE's state itself over 80 applies of a small
/// program. Catches a slower apply path on the reference environment; every other run records it.
#[test]
fn web_ide_in_process_apply_budget() {
    use trust_runtime::web::ide::{IdeRole, WebIdeState};

    let _alone = MEASURING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let project = make_project("in-process-apply");
    let state = WebIdeState::new(Some(project.clone()));
    let session = state
        .create_session(IdeRole::Editor)
        .expect("create editor session");
    let mut version = state
        .open_source(&session.token, "main.st")
        .expect("open source")
        .version;

    let runs = 80_u32;
    let mut total = Duration::ZERO;
    let mut max = Duration::ZERO;
    for index in 0..runs {
        let started = Instant::now();
        let applied = state
            .apply_source(
                &session.token,
                "main.st",
                version,
                format!("PROGRAM Main\nVAR\nA : INT := {index};\nEND_VAR\nEND_PROGRAM\n"),
                true,
            )
            .expect("apply the edit");
        let elapsed = started.elapsed();
        total += elapsed;
        max = max.max(elapsed);
        version = applied.version;
    }
    let average = total / runs;
    let (max_budget, average_budget) = (Duration::from_millis(250), Duration::from_millis(40));
    check_budget("in-process apply max", max < max_budget, max, max_budget);
    check_budget(
        "in-process apply average",
        average < average_budget,
        average,
        average_budget,
    );
    let _ = std::fs::remove_dir_all(project);
}

#[path = "web_ide_integration/web_ide_integration_part_03.rs"]
mod web_ide_integration_part_03;
#[path = "web_ide_integration/web_ide_integration_part_04.rs"]
mod web_ide_integration_part_04;
