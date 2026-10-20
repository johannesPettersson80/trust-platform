#![cfg(feature = "full")]
//! Variables with a direct address at a restart (specification 11 section 6.7, "Directly represented
//! variables at restart"; IEC 61131-3 Ed.3 sections 6.5.6.1 and 6.5.6.2, with the note to 6.5.5.3).
//!
//! A restart initializes a located variable under the rule of its declaration. The initializer sets the
//! variable and writes no byte of the process image. The images are carried over (specification 34 10)
//! and meet the variables again at the next scan: an output image is set from its variable when the
//! scan completes, and a variable located at an input takes the input image when the scan starts.
//! `stateful_lifecycle_sequences.rs` pins the unqualified output through every restart operation; this
//! file adds the retained output and the input. Each case has its own source, so a failure names the
//! one declaration that misbehaves.
//!
//! A variable located at `%M` is bound both ways: the scan loads the marker image into it when it starts
//! and publishes it when it completes. So that the first scan does not replace the initialized variable
//! with a zero or carried image, construction and every restart or online change publish the initial (or
//! retained) value into the marker image before the first scan. The tests that follow the input test pin
//! that for the first start, both restarts, a restart from the store, a retained program variable and an
//! online change. A binding-only update (`bind_direct`) resets no PLC value either, so it publishes the
//! current value of each marker binding that it adds or changes, in the commit that installs the binding
//! and grows the image, and no other marker bit or byte: the last tests of the file pin that.

use std::sync::{Arc, Mutex};

use trust_runtime::error::RuntimeError;
use trust_runtime::harness::{bytecode_bytes_from_source, TestHarness};
use trust_runtime::retain::RetainStore;
use trust_runtime::value::Value;
use trust_runtime::{RestartMode, RetainSnapshot};

/// A retained output word. The program is empty, so only the scan boundary moves values between the
/// variable and the image.
const RETAINED_OUTPUT: &str = r#"
VAR_GLOBAL RETAIN
    kept AT %QW2 : INT := INT#7;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

/// An input word with the initial value 7. The test writes 3 into the image, so the initial value, the
/// carried image and a zeroed image are three different readings.
const INPUT: &str = r#"
VAR_GLOBAL
    level AT %IW0 : INT := INT#7;
END_VAR

PROGRAM Main
END_PROGRAM
"#;

fn global(harness: &TestHarness, name: &str) -> Value {
    harness
        .get_output(name)
        .unwrap_or_else(|| panic!("global {name} exists"))
}

fn image(harness: &TestHarness, address: &str) -> Value {
    harness
        .get_direct_output(address)
        .unwrap_or_else(|error| panic!("image {address} reads: {error}"))
}

fn scan(harness: &mut TestHarness) {
    let cycle = harness.cycle();
    assert!(cycle.errors.is_empty(), "the scan completes: {cycle:?}");
}

/// Spec 11 6.7, directly represented variables at restart: after a restart the retained variable that is
/// located at an output has its retained value in a warm restart and its initial value in a cold
/// restart, and the output image still holds the value of the last completed scan. The first scan after
/// the restart sets the image from the variable.
#[test]
fn restart_gives_a_retained_located_output_its_value_by_mode_and_carries_its_image_over() {
    for (mode, kept_after) in [(RestartMode::Warm, 9), (RestartMode::Cold, 7)] {
        let mut harness = TestHarness::from_source(RETAINED_OUTPUT).expect("fixture compiles");
        assert_eq!(global(&harness, "kept"), Value::Int(7), "{mode:?}");
        assert_eq!(
            image(&harness, "%QW2"),
            Value::Word(0),
            "{mode:?}: the declaration's initializer writes no image byte"
        );

        harness
            .runtime_mut()
            .write_global("kept", Value::Int(9))
            .expect("checked write");
        scan(&mut harness);
        assert_eq!(image(&harness, "%QW2"), Value::Word(9), "{mode:?}");

        harness.restart(mode).expect("restart");
        assert_eq!(
            global(&harness, "kept"),
            Value::Int(kept_after),
            "{mode:?}: a retained variable keeps its value in a warm restart only"
        );
        assert_eq!(
            image(&harness, "%QW2"),
            Value::Word(9),
            "{mode:?}: the restart writes no output image byte"
        );

        scan(&mut harness);
        assert_eq!(
            image(&harness, "%QW2"),
            Value::Word(u16::try_from(kept_after).expect("a small positive value")),
            "{mode:?}: the first scan sets the image from the variable"
        );
    }
}

/// Spec 11 6.7, directly represented variables at restart, and IEC 61131-3 Ed.3 6.5.6.2 (the
/// initialization of system inputs is implementer-specific): the variable located at an input has its
/// initial value after a restart, and the next scan replaces it with the carried input image.
#[test]
fn restart_gives_a_located_input_its_initial_value_until_the_next_scan_samples_the_image() {
    for mode in [RestartMode::Warm, RestartMode::Cold] {
        let mut harness = TestHarness::from_source(INPUT).expect("fixture compiles");
        assert_eq!(global(&harness, "level"), Value::Int(7), "{mode:?}");
        assert_eq!(
            image(&harness, "%IW0"),
            Value::Word(0),
            "{mode:?}: the declaration's initializer writes no image byte"
        );
        scan(&mut harness);
        assert_eq!(
            global(&harness, "level"),
            Value::Int(0),
            "{mode:?}: the first scan takes the (zero) input image"
        );

        harness
            .set_direct_input("%IW0", Value::Word(3))
            .expect("write the input image");
        scan(&mut harness);
        assert_eq!(global(&harness, "level"), Value::Int(3), "{mode:?}");

        harness.restart(mode).expect("restart");
        assert_eq!(
            global(&harness, "level"),
            Value::Int(7),
            "{mode:?}: the restart gives the variable its initial value"
        );
        assert_eq!(
            image(&harness, "%IW0"),
            Value::Word(3),
            "{mode:?}: the restart carries the input image over"
        );

        scan(&mut harness);
        assert_eq!(
            global(&harness, "level"),
            Value::Int(3),
            "{mode:?}: the next scan replaces the initial value with the carried input image"
        );
    }
}

/// A marker word `m` (initial value `initial`) and a marker bit `flag` (initial value TRUE) in a
/// section chosen by the test: "" is unqualified, "NON_RETAIN" and "RETAIN" are the qualified sections.
/// The program only copies what a scan loaded, so `seen` and `seen_flag` show the value that the scan
/// started with.
fn marker_source(qualifier: &str, initial: i16) -> String {
    format!(
        r#"
VAR_GLOBAL {qualifier}
    m AT %MW0 : INT := INT#{initial};
    flag AT %MX2.3 : BOOL := TRUE;
END_VAR

VAR_GLOBAL
    seen : INT := INT#0;
    seen_flag : BOOL := FALSE;
END_VAR

PROGRAM Main
seen := m;
seen_flag := flag;
END_PROGRAM
"#
    )
}

/// A retained marker word that belongs to the program, which a warm restart restores after it has built
/// the new program instance.
const PROGRAM_MARKER: &str = r#"
VAR_GLOBAL
    seen : INT := INT#0;
END_VAR

PROGRAM Main
VAR RETAIN
    pm AT %MW4 : INT := INT#6;
END_VAR
seen := pm;
END_PROGRAM
"#;

/// Writes the marker image the way an outside writer does, between scans.
fn write_marker_word(harness: &mut TestHarness, address: &str, value: u16) {
    harness
        .set_direct_input(address, Value::Word(value))
        .expect("write the marker image");
}

/// Gives `m` the value 100 the way an outside writer does: the marker image is written and the next scan
/// loads it into the variable, which the scan then publishes back.
fn change_marker(harness: &mut TestHarness) {
    write_marker_word(harness, "%MW0", 100);
    scan(harness);
    assert_eq!(
        global(harness, "m"),
        Value::Int(100),
        "the scan loads the image"
    );
    assert_eq!(image(harness, "%MW0"), Value::Word(100));
}

/// The durable store, shared with the test so that it can hold a checkpoint that the live state does not.
struct SharedStore {
    content: Arc<Mutex<RetainSnapshot>>,
}

impl RetainStore for SharedStore {
    fn load(&self) -> Result<RetainSnapshot, RuntimeError> {
        Ok(self.content.lock().expect("store lock").clone())
    }

    fn store(&self, snapshot: &RetainSnapshot) -> Result<(), RuntimeError> {
        *self.content.lock().expect("store lock") = snapshot.clone();
        Ok(())
    }
}

/// Spec 11 6.7, directly represented variables at restart (the `%M` paragraph), and IEC 61131-3 Ed.3
/// 6.5.6: a variable located at `%M` has its initial value at the first start, so the marker image holds
/// that value before the first scan and the scan starts with it. Pre-fix the image was zero and the first
/// scan replaced the initialized variable with 0. The word covers the multi-byte path, the bit the bit
/// path (the other bits of its byte are not touched, which the restart test pins).
#[test]
fn marker_variable_has_its_initial_value_in_the_image_and_the_first_scan_after_first_start() {
    for qualifier in ["", "NON_RETAIN", "RETAIN"] {
        let mut harness =
            TestHarness::from_source(&marker_source(qualifier, 5)).expect("fixture compiles");
        assert_eq!(global(&harness, "m"), Value::Int(5), "'{qualifier}'");
        assert_eq!(
            image(&harness, "%MW0"),
            Value::Word(5),
            "'{qualifier}': construction publishes the initial value into the marker image"
        );
        assert_eq!(
            image(&harness, "%MX2.3"),
            Value::Bool(true),
            "'{qualifier}': a marker bit too"
        );

        scan(&mut harness);
        assert_eq!(
            global(&harness, "seen"),
            Value::Int(5),
            "'{qualifier}': the first scan starts with the initial value"
        );
        assert_eq!(
            global(&harness, "seen_flag"),
            Value::Bool(true),
            "'{qualifier}'"
        );
        assert_eq!(global(&harness, "m"), Value::Int(5), "'{qualifier}'");
    }
}

/// Spec 11 6.7 and IEC 61131-3 Ed.3 6.5.6.1/6.5.6.2: a cold restart gives every variable its initial
/// value, a warm restart gives that value to a `NON_RETAIN` or unqualified variable (IEC_DECISIONS,
/// 2026-07-05). The restart publishes it into the marker image over the carried image, so the first scan
/// starts with it. The image is changed from outside after the last scan, so a carried image and the
/// initial value differ. A marker bit that no variable covers keeps its carried value: the publication
/// touches only the bits and bytes that variables are bound to.
#[test]
fn marker_variable_is_reinitialized_by_a_cold_restart_and_a_warm_restart_without_retain() {
    for (mode, qualifier) in [
        (RestartMode::Cold, ""),
        (RestartMode::Cold, "NON_RETAIN"),
        (RestartMode::Cold, "RETAIN"),
        (RestartMode::Warm, ""),
        (RestartMode::Warm, "NON_RETAIN"),
    ] {
        let case = format!("{mode:?} restart, '{qualifier}'");
        let mut harness =
            TestHarness::from_source(&marker_source(qualifier, 5)).expect("fixture compiles");
        change_marker(&mut harness);
        write_marker_word(&mut harness, "%MW0", 7);
        harness
            .set_direct_input("%MX2.0", Value::Bool(true))
            .expect("write a marker bit that no variable covers");

        harness.restart(mode).expect("restart");
        assert_eq!(
            global(&harness, "m"),
            Value::Int(5),
            "{case}: the variable has its initial value"
        );
        assert_eq!(
            image(&harness, "%MW0"),
            Value::Word(5),
            "{case}: the image holds it, not the carried value"
        );
        assert_eq!(image(&harness, "%MX2.3"), Value::Bool(true), "{case}");
        assert_eq!(
            image(&harness, "%MX2.0"),
            Value::Bool(true),
            "{case}: a bit that no variable covers keeps its carried value"
        );

        scan(&mut harness);
        assert_eq!(
            global(&harness, "seen"),
            Value::Int(5),
            "{case}: the first scan after the restart starts with the initial value"
        );
        assert_eq!(global(&harness, "seen_flag"), Value::Bool(true), "{case}");
    }
}

/// Spec 11 6.7: a warm restart gives a `RETAIN` variable its retained value. For a variable located at
/// `%M` the marker image holds that value, not the image that was carried over, so the first scan starts
/// with it and the retained value is not lost.
#[test]
fn marker_variable_keeps_its_retained_value_in_the_image_through_a_warm_restart() {
    let mut harness =
        TestHarness::from_source(&marker_source("RETAIN", 5)).expect("fixture compiles");
    change_marker(&mut harness);
    write_marker_word(&mut harness, "%MW0", 7);

    harness.restart(RestartMode::Warm).expect("restart");
    assert_eq!(
        global(&harness, "m"),
        Value::Int(100),
        "a retained variable keeps its value in a warm restart"
    );
    assert_eq!(
        image(&harness, "%MW0"),
        Value::Word(100),
        "the image holds the retained value, not the carried one"
    );

    scan(&mut harness);
    assert_eq!(
        global(&harness, "seen"),
        Value::Int(100),
        "the first scan starts with the retained value"
    );
}

/// Spec 11 6.7: a warm restart from the store takes the retained value from the checkpoint. The store
/// holds a value that the live state does not, so the value that the restart publishes into the marker
/// image can only come from applying the checkpoint.
#[test]
fn marker_variable_restarted_from_the_store_has_the_checkpoint_value_in_the_image() {
    let mut harness =
        TestHarness::from_source(&marker_source("RETAIN", 5)).expect("fixture compiles");
    let content = Arc::new(Mutex::new(RetainSnapshot::default()));
    harness.runtime_mut().set_retain_store(
        Some(Box::new(SharedStore {
            content: Arc::clone(&content),
        })),
        None,
    );
    change_marker(&mut harness);
    content
        .lock()
        .expect("store lock")
        .insert("m", Value::Int(42));
    write_marker_word(&mut harness, "%MW0", 7);

    harness
        .restart_with_retain(RestartMode::Warm)
        .expect("restart from the store");
    assert_eq!(
        global(&harness, "m"),
        Value::Int(42),
        "the checkpoint decides the retained value"
    );
    assert_eq!(
        image(&harness, "%MW0"),
        Value::Word(42),
        "the image holds the checkpoint value"
    );

    scan(&mut harness);
    assert_eq!(global(&harness, "seen"), Value::Int(42));
}

/// Spec 11 6.7: a retained variable that belongs to the program is restored after the restart has built
/// the new program instance. The marker image holds the restored value, not the initial value that
/// construction published first.
#[test]
fn retained_program_marker_variable_has_its_retained_value_in_the_image_after_a_warm_restart() {
    let mut harness = TestHarness::from_source(PROGRAM_MARKER).expect("fixture compiles");
    assert_eq!(
        image(&harness, "%MW4"),
        Value::Word(6),
        "construction publishes the initial value"
    );
    write_marker_word(&mut harness, "%MW4", 100);
    scan(&mut harness);
    assert_eq!(global(&harness, "seen"), Value::Int(100));
    write_marker_word(&mut harness, "%MW4", 7);

    harness.restart(RestartMode::Warm).expect("restart");
    assert_eq!(
        image(&harness, "%MW4"),
        Value::Word(100),
        "the image holds the retained value, not the initial or the carried one"
    );
    scan(&mut harness);
    assert_eq!(
        global(&harness, "seen"),
        Value::Int(100),
        "the first scan starts with the retained value"
    );

    harness.restart(RestartMode::Cold).expect("restart");
    assert_eq!(
        image(&harness, "%MW4"),
        Value::Word(6),
        "a cold restart gives the initial value"
    );
}

/// Spec 11 6.7 and 6.7.1: an online change keeps the running value of every variable at the same
/// place, whatever its qualifier (decision OC-1), and publishes it into the marker image before the
/// first scan. The running program's `m` is 100 and the image was overwritten with 7; the new module
/// declares `m` with the initial value 8. For an unqualified, a `NON_RETAIN` and a `RETAIN` `m` alike,
/// the new module holds 100 and so does its image. Wrong behaviour caught: a variable that is not
/// retained reset to the new initial value 8, and an image that keeps the overwritten 7.
#[test]
fn online_change_publishes_the_carried_value_of_a_marker_variable() {
    for (qualifier, expected) in [("", 100), ("NON_RETAIN", 100), ("RETAIN", 100)] {
        let case = format!("online change, '{qualifier}'");
        let mut harness =
            TestHarness::from_source(&marker_source(qualifier, 5)).expect("fixture compiles");
        change_marker(&mut harness);
        write_marker_word(&mut harness, "%MW0", 7);
        let candidate = bytecode_bytes_from_source(&marker_source(qualifier, 8))
            .expect("the candidate compiles");

        harness
            .runtime_mut()
            .apply_online_change_bytes(&candidate, None)
            .expect("the online change is accepted");
        assert_eq!(global(&harness, "m"), Value::Int(expected), "{case}");
        assert_eq!(
            image(&harness, "%MW0"),
            Value::Word(u16::try_from(expected).expect("a small positive value")),
            "{case}: the image holds the variable's carried value, not the overwritten image"
        );

        scan(&mut harness);
        assert_eq!(
            global(&harness, "seen"),
            Value::Int(expected),
            "{case}: the first scan of the new module starts with it"
        );
    }
}

/// Globals with non-zero values that a test binds to a direct address only after admission, as
/// `bind_direct` does. The program copies what the scan loaded into `g`, so `seen` shows the value
/// that the scan started with.
const LATE_BINDING: &str = r#"
VAR_GLOBAL
    g : INT := INT#7;
    h : INT := INT#9;
    flag : BOOL := TRUE;
    seen : INT := INT#0;
END_VAR

PROGRAM Main
seen := g;
END_PROGRAM
"#;

/// A marker word `m` that is bound from the start, a marker byte `tail` that leaves byte 2 of the
/// image covered by no variable, and a global `g` that a test binds later.
const BOUND_MARKER_AND_LATE_GLOBAL: &str = r#"
VAR_GLOBAL
    m AT %MW0 : INT := INT#1;
    tail AT %MB3 : USINT;
    g : INT := INT#7;
    seen_m : INT := INT#0;
    seen_g : INT := INT#0;
END_VAR

PROGRAM Main
seen_m := m;
seen_g := g;
END_PROGRAM
"#;

/// Spec 11 6.7 (the binding-only update): binding a declared variable after admission resets no PLC
/// value. The variable holds 7, the binding grows the marker image from nothing to six bytes, and the
/// commit publishes the 7 into exactly the word it covers. Pre-fix the grown image was zero and the next
/// scan loaded that zero over the variable.
#[test]
fn binding_a_variable_to_a_marker_word_publishes_its_value_into_the_grown_image() {
    let mut harness = TestHarness::from_source(LATE_BINDING).expect("fixture compiles");
    assert!(
        harness.runtime().io().memory().is_empty(),
        "the source declares no marker"
    );

    harness
        .bind_direct("g", "%MW4")
        .expect("bind the global to a marker word");
    assert_eq!(
        harness.runtime().io().memory(),
        &[0, 0, 0, 0, 7, 0],
        "the image grew to cover the word and holds the value in exactly that word"
    );
    assert_eq!(image(&harness, "%MW4"), Value::Word(7));

    scan(&mut harness);
    assert_eq!(
        global(&harness, "g"),
        Value::Int(7),
        "the next scan loads the published value, not zero"
    );
    assert_eq!(global(&harness, "seen"), Value::Int(7));
    assert_eq!(image(&harness, "%MW4"), Value::Word(7));
}

/// Spec 11 6.7: the update publishes the bindings it adds or changes and no other. A driver wrote the
/// marker image after the last scan: a word that another variable is bound to and a byte that no
/// variable covers. The new binding must leave both as they were written, so the next scan loads the
/// written word into `m` instead of the value `m` held at the last scan.
#[test]
fn binding_a_variable_to_markers_leaves_every_other_marker_byte_as_a_driver_wrote_it() {
    let mut harness =
        TestHarness::from_source(BOUND_MARKER_AND_LATE_GLOBAL).expect("fixture compiles");
    scan(&mut harness);
    write_marker_word(&mut harness, "%MW0", 99);
    harness
        .set_direct_input("%MB2", Value::Byte(0x5A))
        .expect("write a marker byte that no variable covers");

    harness
        .bind_direct("g", "%MW4")
        .expect("bind the global to a marker word");
    assert_eq!(
        image(&harness, "%MW4"),
        Value::Word(7),
        "the new binding is published"
    );
    assert_eq!(
        image(&harness, "%MW0"),
        Value::Word(99),
        "a word that another variable covers keeps what the driver wrote"
    );
    assert_eq!(
        image(&harness, "%MB2"),
        Value::Byte(0x5A),
        "a byte that no variable covers keeps what the driver wrote"
    );

    scan(&mut harness);
    assert_eq!(
        global(&harness, "seen_m"),
        Value::Int(99),
        "the scan loads the written word"
    );
    assert_eq!(global(&harness, "seen_g"), Value::Int(7));
}

/// Spec 11 6.7: an update that binds the same variable to the same address again changes no binding, so
/// it publishes nothing. What a driver wrote into the word since the first binding survives it.
#[test]
fn binding_a_variable_again_to_the_same_marker_address_publishes_nothing() {
    let mut harness = TestHarness::from_source(LATE_BINDING).expect("fixture compiles");
    harness
        .bind_direct("g", "%MW4")
        .expect("bind the global to a marker word");
    write_marker_word(&mut harness, "%MW4", 55);

    harness
        .bind_direct("g", "%MW4")
        .expect("bind the global again to the same word");
    assert_eq!(
        image(&harness, "%MW4"),
        Value::Word(55),
        "an unchanged binding is not published again"
    );

    scan(&mut harness);
    assert_eq!(
        global(&harness, "g"),
        Value::Int(55),
        "the scan loads the written word"
    );
}

/// Spec 11 6.7 and 6.4: only a marker binding is published by the update. An output is published by the
/// first scan that completes and an input is sampled by the scan that starts, so binding a variable to
/// either one writes no byte of its image and allocates no marker byte.
#[test]
fn binding_a_variable_to_an_input_or_an_output_publishes_nothing() {
    let mut harness = TestHarness::from_source(LATE_BINDING).expect("fixture compiles");

    harness
        .bind_direct("g", "%QW2")
        .expect("bind the global to an output word");
    harness
        .bind_direct("h", "%IW2")
        .expect("bind the global to an input word");
    assert_eq!(
        image(&harness, "%QW2"),
        Value::Word(0),
        "an output image is published by the first completed scan"
    );
    assert_eq!(
        image(&harness, "%IW2"),
        Value::Word(0),
        "an input image is not published at all"
    );
    assert!(
        harness.runtime().io().memory().is_empty(),
        "no marker byte was allocated or written"
    );

    scan(&mut harness);
    assert_eq!(
        image(&harness, "%QW2"),
        Value::Word(7),
        "the scan publishes the output"
    );
    assert_eq!(
        global(&harness, "h"),
        Value::Int(0),
        "the scan loads the input image over the variable"
    );
}

/// Spec 11 6.7 and 6.4: a hierarchical marker key is outside the flat image but is published and read
/// like a marker byte. The commit inserts the key with the variable's value, so the next scan finds it.
/// Pre-fix the key was absent after the binding and the scan failed to load it.
#[test]
fn binding_a_variable_to_a_hierarchical_marker_publishes_its_value() {
    let mut harness = TestHarness::from_source(LATE_BINDING).expect("fixture compiles");

    harness
        .bind_direct("flag", "%MX1.2.3")
        .expect("bind the global to a hierarchical marker");
    assert_eq!(
        image(&harness, "%MX1.2.3"),
        Value::Bool(true),
        "the binding publishes the variable's value under its key"
    );
    assert!(
        harness.runtime().io().memory().is_empty(),
        "a hierarchical key allocates no flat marker byte"
    );

    scan(&mut harness);
    assert_eq!(global(&harness, "flag"), Value::Bool(true));
    assert_eq!(image(&harness, "%MX1.2.3"), Value::Bool(true));
}
