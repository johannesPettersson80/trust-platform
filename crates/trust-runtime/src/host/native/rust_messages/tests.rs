//! Cargo's JSON messages as the build reads them (specification 48 RU-44). The messages are those a
//! Rust POU build's cargo wrote (`--message-format json`, 18 November 2026), cut to the fields the
//! build reads; the expected places are counted by hand from each line's text.

use super::{read, RustDiagnostic, RustSeverity};

/// The line of the crate's `src/lib.rs` that holds the type error: `"text"` starts after 20
/// characters, at column 21, and its 6 characters end before column 27.
const TYPE_ERROR: &str = r#"{"reason":"compiler-message","package_id":"path+file:///p/rust#rules@0.1.0","target":{"name":"rules"},"message":{"rendered":"error[E0308]: mismatched types\n  --> project/src/lib.rs:10:21\n   |\n10 |     let wrong: u8 = \"text\";\n   |                --   ^^^^^^ expected `u8`, found `&str`\n   |                |\n   |                expected due to this\n\n","$message_type":"diagnostic","children":[],"code":{"code":"E0308","explanation":"Expected type did not match the received type.\n"},"level":"error","message":"mismatched types","spans":[{"file_name":"project/src/lib.rs","byte_start":226,"byte_end":232,"line_start":10,"line_end":10,"column_start":21,"column_end":27,"is_primary":true,"text":[{"text":"    let wrong: u8 = \"text\";","highlight_start":21,"highlight_end":27}],"label":"expected `u8`, found `&str`"},{"file_name":"project/src/lib.rs","byte_start":221,"byte_end":223,"line_start":10,"line_end":10,"column_start":16,"column_end":18,"is_primary":false,"text":[{"text":"    let wrong: u8 = \"text\";","highlight_start":16,"highlight_end":18}],"label":"expected due to this"}]}}"#;

/// The compiler's closing note of a failed crate.
const FAILURE_NOTE: &str = r#"{"reason":"compiler-message","package_id":"path+file:///p/rust#rules@0.1.0","target":{"name":"rules"},"message":{"rendered":"For more information about this error, try `rustc --explain E0308`.\n","$message_type":"diagnostic","children":[],"code":null,"level":"failure-note","message":"For more information about this error, try `rustc --explain E0308`.","spans":[]}}"#;

/// A warning of the generated module, `module.rs` of the build's own package.
const MODULE_WARNING: &str = r#"{"reason":"compiler-message","package_id":"path+file:///p/build/native#trust_module@0.0.0","target":{"name":"trust_module"},"message":{"rendered":"warning: unused imports: `recurse` and `tick`\n --> module.rs:9:36\n  |\n9 | use trust_plc_abi::module::{fault, recurse, run, stmt as st, tick, Areas};\n  |                                    ^^^^^^^                   ^^^^\n","$message_type":"diagnostic","children":[],"code":{"code":"unused_imports","explanation":null},"level":"warning","message":"unused imports: `recurse` and `tick`","spans":[{"file_name":"module.rs","byte_start":1,"byte_end":2,"line_start":9,"line_end":9,"column_start":36,"column_end":43,"is_primary":true,"text":[{"text":"use trust_plc_abi::module::{fault, recurse, run, stmt as st, tick, Areas};","highlight_start":36,"highlight_end":43}],"label":null}]}}"#;

/// A remark of the stack frame of the module's entry, which the stack bound reads.
const REMARK: &str = r#"{"reason":"compiler-message","package_id":"path+file:///p/build/native#trust_module@0.0.0","target":{"name":"trust_module"},"message":{"rendered":"note: <unknown file>:0:0 prolog-epilog (analysis): 56 stack bytes in function 'trust_entry_p0'\n\n","$message_type":"diagnostic","children":[],"code":null,"level":"note","message":"<unknown file>:0:0 prolog-epilog (analysis): 56 stack bytes in function 'trust_entry_p0'","spans":[]}}"#;

/// The diagnostic of the type error that the build reads from `TYPE_ERROR`.
fn type_error() -> RustDiagnostic {
    let messages = read(TYPE_ERROR.as_bytes(), "rust");
    assert_eq!(messages.diagnostics.len(), 1, "{messages:#?}");
    messages.diagnostics[0].clone()
}

#[test]
fn a_type_error_of_the_crate_is_an_error_at_its_place_in_the_project() {
    // RU-44: an error of the project's crate is reported with its `.rs` file, line, column and
    // message. Catches a diagnostic left at the compiler's remapped name `project/src/lib.rs`, or
    // at its secondary span (column 16, the type) instead of the primary one (column 21, the
    // string), or without the compiler's code.
    let found = type_error();
    assert_eq!(found.severity, RustSeverity::Error);
    assert_eq!(found.code.as_deref(), Some("E0308"));
    assert_eq!(found.message, "mismatched types");
    assert_eq!(found.file, "rust/src/lib.rs");
    assert_eq!(
        (found.line, found.column, found.end_line, found.end_column),
        (10, 21, 10, 27)
    );
}

#[test]
fn the_report_names_the_crates_files_as_the_project_does() {
    // RU-44: the report and the diagnostics name the same file. Catches a report that still
    // points at `project/src/lib.rs`, a folder the project does not have.
    let found = type_error();
    assert!(
        found.rendered.contains("\n  --> rust/src/lib.rs:10:21\n"),
        "{}",
        found.rendered
    );
    assert!(!found.rendered.contains("project/"), "{}", found.rendered);
    assert!(
        found
            .rendered
            .starts_with("error[E0308]: mismatched types\n"),
        "{}",
        found.rendered
    );
}

#[test]
fn a_column_counts_utf16_code_units() {
    // Specification 11 §6.8.1c counts a column in UTF-16 code units, as the Problems panel does;
    // the compiler counts characters. In `    let face = "😀"; let wrong: u8 = face;` the second
    // `face` is the compiler's columns 37 to 41: the face before it is one character but two
    // UTF-16 units, so it lies at columns 38 to 42. Catches a column passed on in characters,
    // one short after every character outside the Basic Multilingual Plane.
    let line = r#"    let face = \"😀\"; let wrong: u8 = face;"#;
    let message = format!(
        r#"{{"reason":"compiler-message","message":{{"rendered":"error[E0308]: mismatched types\n","code":{{"code":"E0308"}},"level":"error","message":"mismatched types","spans":[{{"file_name":"project/src/lib.rs","line_start":3,"line_end":3,"column_start":37,"column_end":41,"is_primary":true,"text":[{{"text":"{line}","highlight_start":37,"highlight_end":41}}]}}]}}}}"#
    );
    let messages = read(message.as_bytes(), "rust");
    let found = &messages.diagnostics[0];
    assert_eq!(
        (found.line, found.column, found.end_line, found.end_column),
        (3, 38, 3, 42)
    );
}

#[test]
fn a_warning_of_the_crate_is_a_warning() {
    // RU-44: a crate warning is reported and does not fail the build. Catches a warning taken
    // for an error, or dropped.
    let message = r#"{"reason":"compiler-message","message":{"rendered":"warning: unused variable: `spare`\n --> project/src/lib.rs:12:9\n","code":{"code":"unused_variables","explanation":null},"level":"warning","message":"unused variable: `spare`","spans":[{"file_name":"project/src/lib.rs","line_start":12,"line_end":12,"column_start":9,"column_end":14,"is_primary":true,"text":[{"text":"    let spare = 1;","highlight_start":9,"highlight_end":14}]}]}}"#;
    let messages = read(message.as_bytes(), "rust");
    assert_eq!(messages.diagnostics.len(), 1);
    let found = &messages.diagnostics[0];
    assert_eq!(found.severity, RustSeverity::Warning);
    assert_eq!(found.code.as_deref(), Some("unused_variables"));
    assert_eq!(
        (
            found.file.as_str(),
            found.line,
            found.column,
            found.end_column
        ),
        ("rust/src/lib.rs", 12, 9, 14)
    );
    assert!(found.rendered.contains("--> rust/src/lib.rs:12:9"));
}

#[test]
fn a_file_the_compiler_names_with_backslashes_is_named_with_slashes() {
    // Windows: the compiler joins the remapped `project` and the file with `\`. Catches a
    // diagnostic of a Windows build that keeps `project\src\pou.rs` or is not found at all.
    let message = r#"{"reason":"compiler-message","message":{"rendered":"error[E0425]: cannot find value `x` in this scope\n --> project\\src\\pou.rs:3:5\n","code":{"code":"E0425"},"level":"error","message":"cannot find value `x` in this scope","spans":[{"file_name":"project\\src\\pou.rs","line_start":3,"line_end":3,"column_start":5,"column_end":6,"is_primary":true,"text":[{"text":"    x","highlight_start":5,"highlight_end":6}]}]}}"#;
    let messages = read(message.as_bytes(), "rust");
    assert_eq!(messages.diagnostics[0].file, "rust/src/pou.rs");
    assert!(
        messages.text.contains(" --> rust/src/pou.rs:3:5\n"),
        "{}",
        messages.text
    );
}

#[test]
fn a_rust_first_projects_crate_files_are_named_from_the_project_folder() {
    // Specification 48 RU-65: a Rust-first project is a Cargo package, so its crate is the
    // project's own folder, `.`.
    // Catches its files named `./src/lib.rs` or `/src/lib.rs`, which the Problems panel opens
    // nowhere.
    let messages = read(TYPE_ERROR.as_bytes(), ".");
    assert_eq!(messages.diagnostics[0].file, "src/lib.rs");
    assert!(
        messages.text.contains("\n  --> src/lib.rs:10:21\n"),
        "{}",
        messages.text
    );
}

#[test]
fn the_build_packages_messages_are_reported_but_are_no_diagnostics_of_the_crate() {
    // RU-44 reports the diagnostics of the project's crate. The generated module warns of its own
    // imports on every build, and a failed crate ends with the compiler's note: the report keeps
    // both, in cargo's order, while the crate's diagnostics hold only its error. Catches a
    // Problems panel filled with the generated module's warnings, or a report without them.
    let stdout = [
        r#"{"reason":"compiler-artifact","package_id":"x","target":{"name":"libm"}}"#,
        TYPE_ERROR,
        FAILURE_NOTE,
        MODULE_WARNING,
        "not a JSON line",
        r#"{"reason":"build-finished","success":false}"#,
    ]
    .join("\n");
    let messages = read(stdout.as_bytes(), "rust");
    assert_eq!(messages.diagnostics.len(), 1, "{messages:#?}");
    assert_eq!(messages.diagnostics[0].code.as_deref(), Some("E0308"));
    let error = messages.text.find("error[E0308]").expect("the error");
    let note = messages
        .text
        .find("For more information about this error")
        .expect("the note");
    let module = messages
        .text
        .find("warning: unused imports")
        .expect("the module's warning");
    assert!(error < note && note < module, "{}", messages.text);
    assert!(
        messages.text.contains(" --> module.rs:9:36\n"),
        "{}",
        messages.text
    );
}

#[test]
fn the_stack_remarks_stay_in_the_text_the_stack_bound_reads() {
    // Specification 44 §8.5: the stack bound adds the frame of each function from the compiler's
    // remarks, which now arrive as JSON messages. Catches a build that reads the remarks from
    // cargo's error output, where they no longer are, and bounds the stack without them.
    let messages = read(REMARK.as_bytes(), "rust");
    assert!(messages.diagnostics.is_empty());
    assert_eq!(
        super::super::toolchain::stack_frames(&messages.text),
        vec![("trust_entry_p0".to_string(), 56)]
    );
}
