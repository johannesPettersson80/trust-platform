//! The full map's dependency hygiene policy keeps no list of its own (specification 24): the Rust
//! graphs' advisory exceptions are owned in `deny.toml` alone ("Dependency Exceptions"), which the
//! supply chain gate passes to both `cargo-deny` and `cargo-audit`, and the gate runs `cargo
//! machete` over the repository's own crates ("Unused Dependencies").

use super::DependencyHygienePolicy;

/// The policy's `dependency_hygiene` object with `extra` among its fields.
fn policy_with(extra: &str) -> String {
    format!(
        r#"{{
            {extra}
            "third_party_tiverse_mmap": {{
                "path": "third_party/tiverse-mmap",
                "expected_status": "workspace_exclude",
                "owner": "release engineering",
                "rationale": "patch source",
                "review_date": "2026-04-29"
            }}
        }}"#
    )
}

#[test]
fn a_dependency_policy_with_a_list_of_its_own_is_refused() {
    // Specification 24: deny.toml owns every ignored advisory, and cargo machete runs in the supply
    // chain check. The full map once kept a copy of the advisories that no check held against an
    // audit, and a machete list beside a record saying machete passed, while no gate ran it
    // (checklist G272, G274). Catches such a list accepted again, unread, as serde accepts a field
    // it does not know.
    for (field, list) in [
        (
            "audit_allowlist",
            r#"[{"id": "RUSTSEC-2023-0071", "package": "rsa"}]"#,
        ),
        ("machete_allowlist", "[]"),
    ] {
        let refused = serde_json::from_str::<DependencyHygienePolicy>(&policy_with(&format!(
            r#""{field}": {list},"#
        )))
        .expect_err("a list of its own is refused");
        assert!(
            refused
                .to_string()
                .contains(&format!("unknown field `{field}`")),
            "{refused}"
        );
    }
    serde_json::from_str::<DependencyHygienePolicy>(&policy_with(""))
        .expect("the policy without a list of its own loads");
}
