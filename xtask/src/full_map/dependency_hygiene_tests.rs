//! The full map's dependency hygiene policy keeps no advisory exceptions of its own (specification 24,
//! "Dependency Exceptions"): the Rust graphs' exceptions are owned in `deny.toml` alone, which the supply
//! chain gate passes to both `cargo-deny` and `cargo-audit`.

use super::DependencyHygienePolicy;

/// The policy's `dependency_hygiene` object with `extra` among its fields.
fn policy_with(extra: &str) -> String {
    format!(
        r#"{{
            "third_party_tiverse_mmap": {{
                "path": "third_party/tiverse-mmap",
                "expected_status": "workspace_exclude",
                "owner": "release engineering",
                "rationale": "patch source",
                "review_date": "2026-04-29"
            }},
            {extra}
            "machete_allowlist": []
        }}"#
    )
}

#[test]
fn a_dependency_policy_with_its_own_advisory_list_is_refused() {
    // Specification 24: deny.toml owns every ignored advisory. The full map once kept a copy that no
    // check held against an audit, and it drifted from deny.toml (checklist G272). Catches such a list
    // accepted again, unread, as serde accepts a field it does not know.
    let refused = serde_json::from_str::<DependencyHygienePolicy>(&policy_with(
        r#""audit_allowlist": [{"id": "RUSTSEC-2023-0071", "package": "rsa"}],"#,
    ))
    .expect_err("a second advisory list is refused");
    assert!(
        refused
            .to_string()
            .contains("unknown field `audit_allowlist`"),
        "{refused}"
    );
    serde_json::from_str::<DependencyHygienePolicy>(&policy_with(""))
        .expect("the policy without an advisory list loads");
}
