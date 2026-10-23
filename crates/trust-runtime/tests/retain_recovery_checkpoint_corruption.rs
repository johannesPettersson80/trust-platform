//! Damaged retain checkpoints at the hosted file boundary.
//!
//! Spec 34 RETAIN-02: a restart shall load only a complete, valid, compatible
//! checkpoint. Spec 11 retain fail-closed policy: framing and checksum failures
//! are errors, the complete snapshot is validated before any retained global
//! changes, and a warm restart fails instead of continuing with a partly
//! restored image. Each damaged file must fail with its stable error, leave the
//! running runtime and the stored bytes unchanged, and a later valid checkpoint
//! must still load into the same runtime.

use std::env;
use std::path::{Path, PathBuf};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::TestHarness;
use trust_runtime::retain::{FileRetainStore, RetainStore};
use trust_runtime::value::Value;
use trust_runtime::{RestartMode, RetainSnapshot};

const CORRUPTION_CODE: &str = "runtime_retain_corruption";
/// Byte offset of the payload in the checksum-protected (v3) layout:
/// magic (4) + version (2) + payload length (8).
const PAYLOAD_START: usize = 14;

const SOURCE: &str = r#"
VAR_GLOBAL RETAIN
    count : DINT := DINT#5;
    flag : BOOL := FALSE;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

enum Expected {
    /// The complete corruption message, byte for byte.
    Message(&'static str),
    /// A message whose stable prefix is fixed; the rest carries checksum values.
    Prefix(&'static str),
}

#[derive(Debug, PartialEq)]
struct RunningState {
    count: Option<Value>,
    flag: Option<Value>,
    cycle_counter: u64,
    faulted: bool,
}

fn temp_path(name: &str) -> PathBuf {
    let mut path = env::temp_dir();
    path.push(format!(
        "trust_runtime_retain_recovery_{}_{name}.bin",
        std::process::id()
    ));
    path
}

fn persisted_snapshot() -> RetainSnapshot {
    let mut snapshot = RetainSnapshot::default();
    snapshot.insert("count", Value::DInt(123));
    snapshot.insert("flag", Value::Bool(true));
    snapshot
}

/// A running runtime whose retained globals hold values that differ from the
/// persisted checkpoint, with two completed cycles and the store attached.
fn live_harness(checkpoint: &Path) -> TestHarness {
    let mut harness = TestHarness::from_source(SOURCE).expect("compile retain recovery runtime");
    harness.set_input("count", Value::DInt(77));
    harness.set_input("flag", Value::Bool(false));
    for _ in 0..2 {
        let cycle = harness.cycle();
        assert!(
            cycle.errors.is_empty(),
            "live cycle failed: {:?}",
            cycle.errors
        );
    }
    harness
        .runtime_mut()
        .set_retain_store(Some(Box::new(FileRetainStore::new(checkpoint))), None);
    harness
}

fn running_state(harness: &TestHarness) -> RunningState {
    RunningState {
        count: harness.get_output("count"),
        flag: harness.get_output("flag"),
        cycle_counter: harness.runtime().cycle_counter(),
        faulted: harness.runtime().faulted(),
    }
}

fn valid_checkpoint_bytes(name: &str) -> Vec<u8> {
    let path = temp_path(name);
    let _ = std::fs::remove_file(&path);
    FileRetainStore::new(&path)
        .store(&persisted_snapshot())
        .expect("store valid checkpoint");
    let bytes = std::fs::read(&path).expect("read valid checkpoint");
    let _ = std::fs::remove_file(&path);
    bytes
}

fn assert_expected(error: &RuntimeError, expected: &Expected, name: &str) {
    match (error, expected) {
        (RuntimeError::RetainCorruption(message), Expected::Message(text)) => {
            assert_eq!(message.as_str(), *text, "{name}");
        }
        (RuntimeError::RetainCorruption(message), Expected::Prefix(text)) => {
            assert!(message.starts_with(*text), "{name}: got {message}");
        }
        (other, _) => panic!("{name}: expected RetainCorruption, got {other:?}"),
    }
    assert_eq!(error.stable_code().as_str(), CORRUPTION_CODE, "{name}");
}

#[test]
fn checkpoint_truncated_at_every_offset_fails_closed_and_keeps_running_state() {
    let valid = valid_checkpoint_bytes("truncate_source");
    assert!(
        valid.len() > PAYLOAD_START,
        "checkpoint must carry a payload"
    );
    let path = temp_path("truncate_live");

    // Length 0 is an existing empty file, which must not be read as a missing
    // file (a missing file alone is an empty snapshot).
    for length in 0..valid.len() {
        let damaged = &valid[..length];
        std::fs::write(&path, damaged).expect("write truncated checkpoint");
        let mut harness = live_harness(&path);
        let before = running_state(&harness);

        let load = FileRetainStore::new(&path).load();
        let error = load.expect_err("truncated checkpoint must not load");
        assert_expected(
            &error,
            &Expected::Message("retain data truncated"),
            &format!("length {length}"),
        );

        let restart_error = harness
            .restart_with_retain(RestartMode::Warm)
            .expect_err("warm restart must refuse a truncated checkpoint");
        assert_eq!(restart_error, error, "length {length}");
        assert_eq!(
            running_state(&harness),
            before,
            "length {length}: running state changed"
        );
        assert_eq!(
            std::fs::read(&path).expect("read damaged checkpoint"),
            damaged,
            "length {length}: the stored bytes were rewritten"
        );

        std::fs::write(&path, &valid).expect("restore valid checkpoint");
        harness
            .restart_with_retain(RestartMode::Warm)
            .expect("a later valid checkpoint must load");
        assert_eq!(
            harness.get_output("count"),
            Some(Value::DInt(123)),
            "length {length}"
        );
        assert_eq!(
            harness.get_output("flag"),
            Some(Value::Bool(true)),
            "length {length}"
        );
    }
    let _ = std::fs::remove_file(&path);
}

#[test]
fn damaged_checkpoint_magic_version_trailer_and_checksum_fail_with_stable_errors() {
    let valid = valid_checkpoint_bytes("mutation_source");
    let payload_len =
        u64::from_le_bytes(valid[6..14].try_into().expect("payload length field")) as usize;
    let checksum_offset = PAYLOAD_START + payload_len;
    let trailer_offset = checksum_offset + 4;
    assert_eq!(valid.len(), trailer_offset + 4, "v3 layout");

    let mut magic = valid.clone();
    magic[..4].copy_from_slice(b"XXXX");
    let mut version_four = valid.clone();
    version_four[4..6].copy_from_slice(&4u16.to_le_bytes());
    let mut version_zero = valid.clone();
    version_zero[4..6].copy_from_slice(&0u16.to_le_bytes());
    let mut trailer = valid.clone();
    trailer[trailer_offset..].copy_from_slice(b"XXXX");
    let mut payload = valid.clone();
    payload[PAYLOAD_START] ^= 0x01;
    let mut checksum = valid.clone();
    checksum[checksum_offset] ^= 0x01;
    let mut trailing = valid.clone();
    trailing.push(0);

    let cases: [(&str, Vec<u8>, Expected); 7] = [
        (
            "bad magic",
            magic,
            Expected::Message("invalid retain magic"),
        ),
        (
            "unsupported version 4",
            version_four,
            Expected::Message("unsupported retain version 4"),
        ),
        (
            "unsupported version 0",
            version_zero,
            Expected::Message("unsupported retain version 0"),
        ),
        (
            "bad trailer",
            trailer,
            Expected::Message("invalid retain trailer"),
        ),
        (
            "payload bit flip",
            payload,
            Expected::Prefix("retain checksum mismatch expected "),
        ),
        (
            "checksum bit flip",
            checksum,
            Expected::Prefix("retain checksum mismatch expected "),
        ),
        (
            "trailing byte",
            trailing,
            Expected::Message("retain v3 container has trailing retain data"),
        ),
    ];

    let path = temp_path("mutation_live");
    for (name, bytes, expected) in cases {
        std::fs::write(&path, &bytes).expect("write damaged checkpoint");
        let mut harness = live_harness(&path);
        let before = running_state(&harness);

        let load = FileRetainStore::new(&path).load();
        let error = load.expect_err("damaged checkpoint must not load");
        assert_expected(&error, &expected, name);

        let restart_error = harness
            .restart_with_retain(RestartMode::Warm)
            .expect_err("warm restart must refuse a damaged checkpoint");
        assert_eq!(restart_error, error, "{name}");
        assert_eq!(
            running_state(&harness),
            before,
            "{name}: running state changed"
        );
        assert_eq!(
            std::fs::read(&path).expect("read damaged checkpoint"),
            bytes,
            "{name}: the stored bytes were rewritten"
        );

        std::fs::write(&path, &valid).expect("restore valid checkpoint");
        harness
            .restart_with_retain(RestartMode::Warm)
            .expect("a later valid checkpoint must load");
        assert_eq!(
            harness.get_output("count"),
            Some(Value::DInt(123)),
            "{name}"
        );
    }
    let _ = std::fs::remove_file(&path);
}
