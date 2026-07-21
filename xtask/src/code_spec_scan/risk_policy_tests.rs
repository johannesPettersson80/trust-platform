use super::extract_rust_source;

#[test]
fn rust_risk_policy_marks_parser_derived_boundary_operations_per_fact() {
    let source = r#"
fn boundary() {
    let _now = std::time::SystemTime::now();
    let _token = std::env::var("TOKEN").ok();
    std::thread::spawn(|| work());
    state.insert("ready", true);
    authorize_role("operator");
    persist_retain_and_restart();
}
"#;

    let scan = extract_rust_source("example", source).unwrap();
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.qualified_symbol == "example::boundary")
        .unwrap();

    for expected in [
        "host_time",
        "filesystem_or_environment",
        "subprocess_or_thread",
        "state_mutation",
        "authorization",
        "broad_error_suppression",
        "persistence_retain_reload_restart",
    ] {
        assert!(
            fact.risk_signals.iter().any(|signal| signal == expected),
            "missing {expected} in {:?}",
            fact.risk_signals
        );
    }
}

#[test]
fn rust_risk_policy_does_not_match_result_ok_or_default_substrings() {
    let source = r#"
fn clean() -> Result<(), ()> {
    let _profile = DateTimeProfile::default();
    Ok(())
}
"#;

    let scan = extract_rust_source("example", source).unwrap();
    let fact = scan
        .facts
        .iter()
        .find(|fact| fact.qualified_symbol == "example::clean")
        .unwrap();

    assert!(!fact
        .risk_signals
        .iter()
        .any(|signal| signal == "broad_error_suppression"));
    assert!(!fact
        .risk_signals
        .iter()
        .any(|signal| signal == "fault_mapping"));
}
