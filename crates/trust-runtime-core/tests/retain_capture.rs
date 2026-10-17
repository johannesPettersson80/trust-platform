//! Specification 11 §6.7 and specification 34 §10 (bounded-C slice C5-2): the retain capture that a
//! computer and a chip share.
//!
//! The corpus `retain` fixture is the artifact: `VAR_GLOBAL RETAIN counter : DINT := DINT#0;
//! setpoint : REAL := REAL#20.0;` and the program `Retained`, instantiated as `Plant`, with
//! `runs : DINT` and `label : STRING[12] := 'boiler'`, all advanced by one 10 ms scan per cycle.
//!
//! The capture is checked at the boundary the engine gives it: a hook stages the engine's
//! pre-output-sample checkpoint into the shared capture through `CycleAccess::capture_retain`, so
//! the staged STRN v3 payload holds the finished scan's values and not a value a later host write
//! changed. The rule is checked against the decisions the hosted `RetainManager` made before the
//! rule moved into the capture.

use std::cell::RefCell;
use std::rc::Rc;

use trust_runtime_core::{
    error::RuntimeError,
    retain::{capture::RetainCapture, codec::encode_snapshot, RetainSnapshot},
    value::{Duration, Value},
    vm::{CycleAccess, CycleHooks, CyclePhase, PreparationLimits, PreparedModule},
};

const RETAIN_BYTES: &[u8] =
    include_bytes!("../../trust-runtime/tests/fixtures/bounded-c/retain/program.stbc");

/// Captures the finished scan at the pre-output-sample checkpoint, then a late host write at
/// `AfterOutputSample` changes a retained value the capture must not include.
struct Boundary {
    capture: Rc<RefCell<RetainCapture>>,
    late_write: bool,
}

impl CycleHooks for Boundary {
    fn on_phase(
        &mut self,
        phase: CyclePhase,
        access: &mut CycleAccess<'_, '_>,
    ) -> Result<(), RuntimeError> {
        match phase {
            CyclePhase::BeforeOutputSample => {
                access.capture_retain(&mut self.capture.borrow_mut())?;
            }
            CyclePhase::AfterOutputSample => {
                access.write_named_path("counter", &[], Value::DInt(99))?;
                self.late_write = true;
            }
            _ => {}
        }
        Ok(())
    }
}

/// The STRN v3 payload the codec writes for `entries`, the bytes a capture must stage.
fn payload_of(entries: &[(&str, Value)]) -> Vec<u8> {
    let mut snapshot = RetainSnapshot::default();
    for (name, value) in entries {
        snapshot.insert(*name, value.clone());
    }
    let mut bytes = Vec::new();
    snapshot
        .encode_snapshot_payload_into(&mut bytes)
        .expect("the expected retained payload encodes");
    bytes
}

/// Stage `entries` as a capture's payload, as the engine's capture appends them.
fn stage(capture: &mut RetainCapture, entries: &[(&str, Value)]) {
    let mut snapshot = RetainSnapshot::default();
    for (name, value) in entries {
        snapshot.insert(*name, value.clone());
    }
    capture
        .stage_values(snapshot.values())
        .expect("the retained payload stages");
    assert_eq!(
        capture.staged_payload(),
        payload_of(entries).as_slice(),
        "staging the values writes the codec's payload"
    );
}

/// Specification 11 §6.7 and specification 34 §10: the platform's capture is taken at the
/// established pre-output-sample checkpoint, so it holds the values of the finished scan — here the
/// first scan's `counter = 1` — and not the value a later host write changed.
#[test]
fn capture_is_taken_at_before_output_sample() {
    let prepared = PreparedModule::from_bytes(RETAIN_BYTES, PreparationLimits::default())
        .expect("the shared admission accepts the corpus retain fixture");
    let mut state = prepared
        .instantiate(0)
        .expect("the corpus retain fixture constructs");
    let capture = Rc::new(RefCell::new(RetainCapture::new()));
    let mut hooks = Boundary {
        capture: Rc::clone(&capture),
        late_write: false,
    };
    state
        .execute_cycle_with_hooks(Duration::from_millis(10), &mut hooks)
        .expect("the first scan succeeds");
    assert!(
        hooks.late_write,
        "the late host write runs after the checkpoint"
    );

    // Worked out by hand from the corpus fixture: two retained globals and the program `Retained`
    // instantiated as `Plant` with two retained variables, each advanced once by the first scan.
    let expected = payload_of(&[
        ("counter", Value::DInt(1)),
        ("setpoint", Value::Real(20.5)),
        ("@program/Plant/runs", Value::DInt(1)),
        ("@program/Plant/label", Value::String("boiler".into())),
    ]);
    assert_eq!(
        capture.borrow().staged_payload(),
        expected.as_slice(),
        "the checkpoint holds the finished scan's values, not the later host write"
    );
    assert_eq!(
        state.storage().get_global("counter"),
        Some(&Value::DInt(99))
    );
}

/// Specification 11 §6.7: an image equal to the written one is not saved again — the checkpoint is
/// still taken — while a changed value or a changed physical representation is a changed image.
#[test]
fn an_unchanged_image_is_not_saved_again() {
    let mut capture = RetainCapture::new();
    capture.configure(Some(Duration::from_millis(100)), Duration::from_millis(10));
    capture.mark_dirty();
    assert!(capture.should_save(Duration::from_millis(110)));

    // No image was written yet, so the first capture always has to be written.
    stage(
        &mut capture,
        &[("counter", Value::DInt(1)), ("setpoint", Value::Real(20.5))],
    );
    assert!(capture.image_changed());
    capture.accept(Duration::from_millis(110));

    // The same retained storage again: the checkpoint is taken without a write.
    capture.mark_dirty();
    assert!(capture.should_save(Duration::from_millis(210)));
    stage(
        &mut capture,
        &[("counter", Value::DInt(1)), ("setpoint", Value::Real(20.5))],
    );
    assert!(
        !capture.image_changed(),
        "an unchanged image must not be saved again"
    );
    capture.unchanged(Duration::from_millis(210));
    assert_eq!(capture.last_save(), Duration::from_millis(210));
    assert!(!capture.is_dirty());

    // A changed value is a changed image.
    stage(
        &mut capture,
        &[("counter", Value::DInt(2)), ("setpoint", Value::Real(20.5))],
    );
    assert!(
        capture.image_changed(),
        "a changed value is a changed image"
    );
    capture.accept(Duration::from_millis(210));

    // A signed-zero REAL equals 0.0 logically but is different retained storage.
    stage(
        &mut capture,
        &[("counter", Value::DInt(2)), ("setpoint", Value::Real(-0.0))],
    );
    capture.accept(Duration::from_millis(210));
    stage(
        &mut capture,
        &[("counter", Value::DInt(2)), ("setpoint", Value::Real(0.0))],
    );
    assert!(
        capture.image_changed(),
        "0.0 and -0.0 are different retained storage"
    );
}

/// Specification 11 §6.7 and specification 34 §10: for one interval and a sequence of logical
/// times, the shared capture answers the due question exactly as the hosted `RetainManager`'s
/// `should_save` did before the rule moved here (worked out from its arithmetic: the cadence runs
/// from the configuration or the last checkpoint, never from the dirty mark, and a logical time
/// before the cadence start never makes the interval due early).
#[test]
fn the_due_rule_equals_the_retain_managers() {
    let mut capture = RetainCapture::new();
    // No interval is configured: never due, however dirty.
    capture.mark_dirty();
    assert!(!capture.should_save(Duration::ZERO));
    assert!(!capture.should_save(Duration::from_secs(1)));

    // An interval starts its cadence at configuration, where nothing is dirty yet.
    capture.configure(Some(Duration::from_millis(100)), Duration::from_millis(10));
    assert!(!capture.should_save(Duration::from_millis(1_000)));
    capture.mark_dirty();
    // The cadence runs from the configuration time, not from the dirty mark.
    assert!(!capture.should_save(Duration::from_millis(109)));
    assert!(capture.should_save(Duration::from_millis(110)));
    // A taken checkpoint clears the dirty flag and restarts the cadence.
    capture.unchanged(Duration::from_millis(110));
    assert!(!capture.should_save(Duration::from_millis(110)));
    capture.mark_dirty();
    assert!(!capture.should_save(Duration::from_millis(209)));
    assert!(capture.should_save(Duration::from_millis(210)));

    // A zero interval is due as soon as the values are dirty.
    capture.set_interval(Some(Duration::ZERO));
    capture.unchanged(Duration::from_millis(210));
    capture.mark_dirty();
    assert!(capture.should_save(Duration::from_millis(210)));

    // A logical time earlier than the cadence start never makes the interval due early.
    capture.configure(Some(Duration::from_millis(100)), Duration::from_millis(500));
    capture.mark_dirty();
    assert!(!capture.should_save(Duration::from_millis(400)));
    assert!(capture.should_save(Duration::from_millis(600)));
}

/// Specification 11 §6.7 and specification 34 §10.1: appending a snapshot's payload writes exactly
/// the payload bytes `encode_snapshot` frames, so a chip's staged bytes and a computer's retain
/// file hold the same snapshot.
///
/// The snapshot is the known one of `retain_codec.rs`: `b = TRUE`, `i = INT#-2`, `s = "ab"`, whose
/// 31-byte payload is fixed by the codec. A writer that dropped the count, swapped two tags or
/// mis-framed a name would not write these bytes.
#[test]
fn appending_a_payload_writes_the_same_bytes_as_encoding_it() {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("b", Value::Bool(true));
    snapshot.insert("i", Value::Int(-2));
    snapshot.insert("s", Value::String("ab".into()));

    // Worked out by hand from the format the code defines: the value count, then each name and
    // tagged value. 4 count + (5 name + 2 BOOL) + (5 name + 3 INT) + (5 name + 7 STRING) = 31.
    let hand_written: [u8; 31] = [
        0x03, 0x00, 0x00, 0x00, // value count 3 (u32 LE)
        0x01, 0x00, 0x00, 0x00, 0x62, // name "b": length 1 (u32 LE) + 'b'
        0x01, 0x01, // tag BOOL 1 + true 1
        0x01, 0x00, 0x00, 0x00, 0x69, // name "i": length 1 + 'i'
        0x03, 0xfe, 0xff, // tag INT 3 + -2 (i16 LE)
        0x01, 0x00, 0x00, 0x00, 0x73, // name "s": length 1 + 's'
        0x18, 0x02, 0x00, 0x00, 0x00, 0x61, 0x62, // tag STRING 24 + length 2 + "ab"
    ];

    let appended = {
        let mut bytes = Vec::new();
        snapshot
            .encode_snapshot_payload_into(&mut bytes)
            .expect("append the known snapshot's payload");
        bytes
    };
    assert_eq!(
        appended, hand_written,
        "the appended payload is the codec's known payload"
    );

    let file = encode_snapshot(&snapshot).expect("encode the known snapshot");
    // The framed payload sits between the four-byte magic, the two-byte version and the eight-byte
    // length, and the four-byte payload CRC-32 and the four-byte trailer.
    let framed = &file[14..14 + hand_written.len()];
    assert_eq!(
        appended, framed,
        "appending a payload writes the same bytes as encoding them"
    );
}
