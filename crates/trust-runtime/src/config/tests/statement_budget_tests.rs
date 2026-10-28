use super::*;

/// Specification 44 NX-10: `[resource] max_statements_per_scan` is the statement budget of a task
/// activation, 1,000,000 when the project sets none, 500 when it sets 500; 0 and a value above the
/// largest 32-bit budget are refused. Catches a setting the parser drops, a default other than the
/// specification's, and a budget that can never run a statement.
#[test]
fn runtime_schema_reads_the_statement_budget() {
    let default = parse_runtime_toml_from_text(&runtime_toml(), "runtime.toml")
        .expect("runtime config should parse");
    assert_eq!(default.max_statements_per_scan, 1_000_000);

    let with_budget = |budget: &str| {
        runtime_toml().replace(
            "cycle_interval_ms = 100\n",
            &format!("cycle_interval_ms = 100\nmax_statements_per_scan = {budget}\n"),
        )
    };
    let set = parse_runtime_toml_from_text(&with_budget("500"), "runtime.toml")
        .expect("a budget of 500 should parse");
    assert_eq!(set.max_statements_per_scan, 500);
    for refused in ["0", "4294967296"] {
        let error = parse_runtime_toml_from_text(&with_budget(refused), "runtime.toml")
            .expect_err("the budget is refused");
        assert!(
            error
                .to_string()
                .contains("resource.max_statements_per_scan must be between 1 and 4294967295"),
            "{refused}: {error}"
        );
    }
}
