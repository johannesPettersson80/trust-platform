//! The sidebar's gates (specification 25 §6.6). The first three cases are the extension's former
//! `compileGate.ts` assertions, moved here with the rule.

use super::*;

fn gate(argument: Value) -> Value {
    let answer = answer(argument).expect("answer");
    assert_eq!(answer["ok"], json!(true));
    answer
}

#[test]
fn source_errors_in_problems_block_with_their_count() {
    // Two errors in one source: every verb names them. Catches: a count of files instead of errors.
    let answer = gate(json!({ "diagnostics": [{ "path": "/p/src/Main.st", "errors": 2 }] }));
    assert_eq!(
        answer["reasons"]["update"],
        json!("Fix 2 errors to update.")
    );
    assert_eq!(answer["reasons"]["start"], json!("Fix 2 errors to start."));
    assert_eq!(
        answer["validity"]["label"],
        json!("2 errors — see Problems")
    );
    assert_eq!(answer["validity"]["sourceErrors"], json!(2));
}

#[test]
fn a_settings_error_in_problems_names_runtime_toml() {
    // One error in runtime.toml (any letter case, either separator): the settings reason, even with a
    // source error beside it. Catches: settings errors counted as source errors.
    let answer = gate(json!({ "diagnostics": [
        { "path": "C:\\p\\Runtime.TOML", "errors": 1 },
        { "path": "/p/src/Main.st", "errors": 4 },
    ] }));
    assert_eq!(
        answer["reasons"]["start"],
        json!("Fix runtime.toml to start.")
    );
    assert_eq!(answer["validity"]["configErrors"], json!(1));
    assert_eq!(answer["validity"]["errors"], json!(5));
}

#[test]
fn a_failed_compile_blocks_when_problems_shows_none() {
    // No error in Problems; the last Compile failed with three source errors.
    let answer = gate(json!({
        "diagnostics": [],
        "compile": { "state": "failed", "report": {
            "errors": 3,
            "issues": [
                { "severity": "error", "code": "compile", "file": "/p/src/Main.st" },
                { "severity": "error", "code": "compile" },
                { "severity": "error", "code": "sources" },
                { "severity": "warning", "code": "config.resource" },
            ],
        } },
    }));
    assert_eq!(answer["reasons"]["debug"], json!("Fix 3 errors to debug."));
    assert_eq!(answer["validity"]["label"], json!("No known errors"));
    assert_eq!(
        answer["compile"],
        json!({ "errors": 3, "sourceErrors": 3, "configErrors": 0 }),
        "a warning is no error"
    );
}

#[test]
fn a_compile_error_about_the_settings_names_runtime_toml() {
    // `config.resource` (the RESOURCE of runtime.toml) and an issue in trust-lsp.toml are settings
    // errors. Catches: a code that names the configuration counted as a source error.
    for issue in [
        json!({ "severity": "Error", "code": "config.resource" }),
        json!({ "severity": "error", "code": "compile", "file": "/p/trust-lsp.toml" }),
    ] {
        let answer = gate(json!({
            "compile": { "state": "failed", "report": { "errors": 1, "issues": [issue] } },
        }));
        assert_eq!(
            answer["reasons"]["update"],
            json!("Fix runtime.toml to update.")
        );
        assert_eq!(answer["compile"]["configErrors"], json!(1));
    }
}

#[test]
fn a_clean_dirty_or_unknown_compile_and_other_files_do_not_block() {
    // An error in README.md is not a project error; a Compile that did not fail blocks nothing.
    for state in ["clean", "dirty", "unknown"] {
        let answer = gate(json!({
            "diagnostics": [{ "path": "/p/README.md", "errors": 9 }],
            "compile": { "state": state },
        }));
        assert_eq!(answer["reasons"]["start"], Value::Null, "{state}");
        assert_eq!(answer["validity"]["ok"], json!(true), "{state}");
        assert_eq!(answer["compile"], Value::Null, "{state}");
    }
}

#[test]
fn one_error_is_singular_and_a_failed_compile_without_a_report_counts_one() {
    let answer = gate(json!({ "diagnostics": [{ "path": "/p/src/a.POU", "errors": 1 }] }));
    assert_eq!(answer["reasons"]["start"], json!("Fix 1 error to start."));
    assert_eq!(answer["validity"]["label"], json!("1 error — see Problems"));
    let answer = gate(json!({ "compile": { "state": "failed" } }));
    assert_eq!(answer["reasons"]["start"], json!("Fix 1 error to start."));
}

#[test]
fn debug_needs_a_reachable_target_and_no_gate() {
    // Catches: Debug offered while the target starts or cannot be reached.
    for (status, enabled, title) in [
        ("running", true, "Start debugging"),
        ("stopped", true, "Start debugging"),
        (
            "starting",
            false,
            "Debug is unavailable until the target is reachable.",
        ),
        (
            "unreachable",
            false,
            "Debug is unavailable until the target is reachable.",
        ),
    ] {
        let answer = gate(json!({ "target": { "status": status } }));
        assert_eq!(answer["debug"]["enabled"], json!(enabled), "{status}");
        assert_eq!(answer["debug"]["title"], json!(title), "{status}");
    }
    let gated = gate(json!({
        "diagnostics": [{ "path": "/p/src/Main.st", "errors": 2 }],
        "target": { "status": "running" },
    }));
    assert_eq!(gated["debug"]["enabled"], json!(false));
    assert_eq!(gated["debug"]["title"], json!("Fix 2 errors to debug."));
}
