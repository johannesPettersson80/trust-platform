//! Failure injection: a candidate refused at each reachable installation step leaves the live
//! controller unchanged.
//!
//! Spec 10 section 4.7 and 4.8, spec 34 UPDATE-01 and spec 44 §10.2 and §12: a refused candidate
//! keeps the live forces, queued debug writes, published outputs, cycle counter and the running
//! program, and the live controller still runs afterwards.
//!
//! The tests need the toolchain of specification 44 §4.6 at TRUST_TOOLCHAIN.

use trust_runtime::debug::{DebugControl, ForcedVarTarget};
use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{SourceFile, TestHarness};
use trust_runtime::io::IoAddress;
use trust_runtime::native::{build, Options, Target, Toolchain};
use trust_runtime::runtime_core::native::image::Image;
use trust_runtime::value::Value;
use trust_runtime::CompiledRuntime;

const LIVE_SOURCE: &str = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
VAR published AT %QW0 : INT; END_VAR
count := count + INT#1;
published := count;
END_PROGRAM
"#;

const CANDIDATE_SOURCE: &str = r#"
VAR_GLOBAL count : INT := INT#0; hold : INT := INT#0; END_VAR
PROGRAM Main
VAR published AT %QW0 : INT; END_VAR
count := count + INT#10;
published := count;
END_PROGRAM
"#;

/// A live controller after one cycle, with count forced to 99 and a queued write to hold.
struct Live {
    harness: TestHarness,
    debug: DebugControl,
}

fn live() -> Live {
    let mut harness = TestHarness::from_source(LIVE_SOURCE).expect("compile live source");
    let first = harness.cycle();
    assert!(
        first.errors.is_empty(),
        "live cycle failed: {:?}",
        first.errors
    );
    let debug = DebugControl::new();
    harness.runtime_mut().set_debug_control(debug.clone());
    debug
        .try_force_global("count", Value::Int(99))
        .expect("a new force is inside the bound");
    debug
        .try_enqueue_global_write("hold", Value::Int(7))
        .expect("a new target is inside the bound");
    Live { harness, debug }
}

/// Everything a refused candidate must leave untouched.
struct Observed {
    forced_count: Option<Value>,
    output: Result<Value, RuntimeError>,
    cycle_counter: u64,
}

fn observe(runtime: &CompiledRuntime, debug: &DebugControl) -> Observed {
    Observed {
        forced_count: debug.forced_var_value(&ForcedVarTarget::Global("count".into())),
        output: runtime.read_io(&IoAddress::parse("%QW0").expect("output word address")),
        cycle_counter: runtime.cycle_counter(),
    }
}

fn assert_unchanged(before: &Observed, after: &Observed, label: &str) {
    assert_eq!(
        after.forced_count, before.forced_count,
        "{label}: the active force changed"
    );
    assert_eq!(
        after.output, before.output,
        "{label}: the published output image changed"
    );
    assert_eq!(
        after.cycle_counter, before.cycle_counter,
        "{label}: the cycle counter changed"
    );
}

/// Run live cycles after a refusal: the queued write must apply, the force must hold, and once the
/// force is released each scan adds 1, as the live program does, and not 10, as the candidate
/// does.
fn assert_live_runtime_still_runs(live: &mut Live, label: &str) {
    assert_eq!(
        live.harness.runtime().read_variable("Main.published"),
        Ok(Value::Int(1)),
        "{label}: the published output must still be cycle 1's"
    );
    let cycle = live.harness.cycle();
    assert!(
        cycle.errors.is_empty(),
        "{label}: the live runtime must still run: {:?}",
        cycle.errors
    );
    assert_eq!(
        live.harness.runtime().read_variable("hold"),
        Ok(Value::Int(7)),
        "{label}: the queued write must survive the refused candidate"
    );
    assert_eq!(
        live.debug
            .forced_var_value(&ForcedVarTarget::Global("count".into())),
        Some(Value::Int(99)),
        "{label}: the force must survive the refused candidate"
    );
    live.debug.release_global("count");
    let mut counts = Vec::new();
    for _ in 0..2 {
        let cycle = live.harness.cycle();
        assert!(
            cycle.errors.is_empty(),
            "{label}: the live runtime must run after the release: {:?}",
            cycle.errors
        );
        match live.harness.runtime().read_variable("count") {
            Ok(Value::Int(count)) => counts.push(count),
            other => panic!("{label}: count must be an INT, found {other:?}"),
        }
    }
    assert_eq!(
        counts[1] - counts[0],
        1,
        "{label}: a scan of the live program adds 1, the candidate's would add 10"
    );
}

fn refused_candidate(live: &mut Live, bytes: &[u8]) -> RuntimeError {
    match live.harness.runtime_mut().apply_online_change_bytes(bytes) {
        Ok(()) => panic!("the candidate must be refused"),
        Err(error) => error,
    }
}

/// The image of `CANDIDATE_SOURCE` for this computer.
fn candidate_bytes() -> Vec<u8> {
    let toolchain =
        Toolchain::find().expect("TRUST_TOOLCHAIN names the toolchain of specification 44 §4.6");
    let work = std::env::temp_dir().join(format!(
        "trust-candidate-refusal-{}-{:?}",
        std::process::id(),
        std::thread::current().id()
    ));
    let built = build(
        &[SourceFile::new(CANDIDATE_SOURCE)],
        &Options {
            target: Target::this_cpu().expect("a computer target"),
            hooks: true,
        },
        &toolchain,
        &work,
    )
    .expect("build candidate image");
    let _ = std::fs::remove_dir_all(&work);
    built.image
}

#[test]
fn bad_magic_candidate_is_refused_at_decode_with_the_live_state_unchanged() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    let mut bytes = candidate_bytes();
    bytes[0] = 0x00;
    let error = refused_candidate(&mut live, &bytes);
    assert!(
        matches!(&error, RuntimeError::InvalidBundle(text) if text.contains("TRSTI001")),
        "a candidate without the image magic is refused naming it: {error:?}"
    );
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "bad magic",
    );
    assert_live_runtime_still_runs(&mut live, "bad magic");
}

#[test]
fn truncated_candidate_is_refused_with_exactly_the_decoder_error() {
    let mut live = live();
    let before = observe(live.harness.runtime(), &live.debug);
    let bytes = candidate_bytes();
    let truncated = &bytes[..bytes.len() / 2];
    let Err(decode_error) = Image::parse(truncated) else {
        panic!("a truncated candidate must not decode");
    };
    let error = refused_candidate(&mut live, truncated);
    assert!(
        matches!(&error, RuntimeError::InvalidBundle(text) if text.contains(&decode_error.to_string())),
        "a truncated candidate must fail with the decoder's own refusal ({decode_error}): {error:?}"
    );
    assert_unchanged(
        &before,
        &observe(live.harness.runtime(), &live.debug),
        "truncated candidate",
    );
    assert_live_runtime_still_runs(&mut live, "truncated candidate");
}
