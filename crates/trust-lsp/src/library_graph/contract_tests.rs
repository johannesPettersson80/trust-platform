use super::*;
use std::path::{Path, PathBuf};

fn config(contents: &str) -> ProjectConfig {
    let root = Path::new("/workspace/library-graph");
    ProjectConfig::from_contents(root, Some(root.join("trust-lsp.toml")), contents)
}

fn issues_with_code<'a>(issues: &'a [LibraryIssue], code: &str) -> Vec<&'a LibraryIssue> {
    issues.iter().filter(|issue| issue.code == code).collect()
}

#[test]
fn empty_configuration_builds_empty_graph_without_issues() {
    let config = config("");
    assert!(build_library_graph(&config).nodes.is_empty());
    assert!(library_dependency_issues(&config).is_empty());
}

#[test]
fn graph_node_order_follows_configuration_order() {
    let config = config(
        r#"
[[libraries]]
name = "Zulu"
path = "libs/z"

[[libraries]]
name = "Alpha"
path = "libs/a"

[[libraries]]
name = "Middle"
path = "libs/m"
"#,
    );
    let graph = build_library_graph(&config);
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.name.as_str())
            .collect::<Vec<_>>(),
        vec!["Zulu", "Alpha", "Middle"]
    );
}

#[test]
fn graph_nodes_preserve_version_path_and_dependency_contract() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
version = "3.0"
dependencies = [{ name = "Core", version = "2.0" }, "Utils"]

[[libraries]]
name = "Core"
path = "libs/core"
version = "2.0"

[[libraries]]
name = "Utils"
path = "libs/utils"
"#,
    );
    let graph = build_library_graph(&config);
    let app = &graph.nodes[0];
    assert_eq!(app.name, "Application");
    assert_eq!(app.version.as_deref(), Some("3.0"));
    assert_eq!(
        app.path,
        PathBuf::from("/workspace/library-graph/libs/application")
    );
    assert_eq!(app.dependencies.len(), 2);
    assert_eq!(app.dependencies[0].name, "Core");
    assert_eq!(app.dependencies[0].version.as_deref(), Some("2.0"));
    assert_eq!(app.dependencies[1].name, "Utils");
    assert!(app.dependencies[1].version.is_none());
}

#[test]
fn absent_dependency_emits_one_l001_with_exact_subject() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = ["Missing"]
"#,
    );
    let issues = library_dependency_issues(&config);
    let missing = issues_with_code(&issues, "L001");
    assert_eq!(missing.len(), 1);
    assert_eq!(missing[0].subject, "Application");
    assert_eq!(missing[0].dependency.as_deref(), Some("Missing"));
    assert!(missing[0].message.contains("not configured"));
}

#[test]
fn duplicate_absent_dependency_edges_emit_one_l001() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = ["Missing", "missing", "Missing"]
"#,
    );
    let issues = library_dependency_issues(&config);
    assert_eq!(issues_with_code(&issues, "L001").len(), 1);
}

#[test]
fn unversioned_requirement_accepts_any_configured_version() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = ["Core"]

[[libraries]]
name = "Core"
path = "libs/core"
version = "9.7"
"#,
    );
    assert!(library_dependency_issues(&config).is_empty());
}

#[test]
fn exact_version_requirement_accepts_matching_candidate() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = [{ name = "Core", version = "2.0" }]

[[libraries]]
name = "Core"
path = "libs/core-v1"
version = "1.0"

[[libraries]]
name = "Core"
path = "libs/core-v2"
version = "2.0"
"#,
    );
    let issues = library_dependency_issues(&config);
    assert!(issues_with_code(&issues, "L002").is_empty());
}

#[test]
fn version_mismatch_emits_l002_with_sorted_available_versions() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = [{ name = "Core", version = "3.0" }]

[[libraries]]
name = "Core"
path = "libs/core-v2"
version = "2.0"

[[libraries]]
name = "Core"
path = "libs/core-v1"
version = "1.0"
"#,
    );
    let issues = library_dependency_issues(&config);
    let mismatches = issues_with_code(&issues, "L002");
    assert_eq!(mismatches.len(), 1);
    assert_eq!(mismatches[0].subject, "Application");
    assert_eq!(mismatches[0].dependency.as_deref(), Some("Core"));
    assert!(mismatches[0].message.contains("1.0, 2.0"));
}

#[test]
fn library_and_dependency_identities_are_ascii_case_insensitive() {
    let config = config(
        r#"
[[libraries]]
name = "Application"
path = "libs/application"
dependencies = [{ name = "core", version = "2.0" }]

[[libraries]]
name = "CORE"
path = "libs/core"
version = "2.0"
"#,
    );
    assert!(library_dependency_issues(&config).is_empty());
}

#[test]
fn duplicate_same_version_declarations_do_not_conflict() {
    let config = config(
        r#"
[[libraries]]
name = "Core"
path = "libs/core-a"
version = "2.0"

[[libraries]]
name = "core"
path = "libs/core-b"
version = "2.0"
"#,
    );
    assert!(issues_with_code(&library_dependency_issues(&config), "L003").is_empty());
}

#[test]
fn conflicting_versions_emit_one_deterministic_l003() {
    let config = config(
        r#"
[[libraries]]
name = "Core"
path = "libs/core-v2"
version = "2.0"

[[libraries]]
name = "core"
path = "libs/core-v1"
version = "1.0"

[[libraries]]
name = "CORE"
path = "libs/core-v3"
version = "3.0"
"#,
    );
    let issues = library_dependency_issues(&config);
    let conflicts = issues_with_code(&issues, "L003");
    assert_eq!(conflicts.len(), 1);
    assert_eq!(conflicts[0].subject, "Core");
    assert!(conflicts[0].message.contains("1.0, 2.0, 3.0"));
}

#[test]
fn unspecified_and_explicit_versions_are_conflicting_declarations() {
    let config = config(
        r#"
[[libraries]]
name = "Core"
path = "libs/core-unspecified"

[[libraries]]
name = "Core"
path = "libs/core-v1"
version = "1.0"
"#,
    );
    let issues = library_dependency_issues(&config);
    let conflicts = issues_with_code(&issues, "L003");
    assert_eq!(conflicts.len(), 1);
    assert!(conflicts[0].message.contains("1.0, unspecified"));
}

#[test]
fn self_dependency_emits_canonical_l004() {
    let config = config(
        r#"
[[libraries]]
name = "Self"
path = "libs/self"
dependencies = ["Self"]
"#,
    );
    let issues = library_dependency_issues(&config);
    let cycles = issues_with_code(&issues, "L004");
    assert_eq!(cycles.len(), 1);
    assert_eq!(cycles[0].subject, "Self");
    assert!(cycles[0].message.ends_with("Self -> Self"));
}

#[test]
fn three_node_cycle_starts_at_lexicographically_smallest_identity() {
    let config = config(
        r#"
[[libraries]]
name = "Zulu"
path = "libs/z"
dependencies = ["Middle"]

[[libraries]]
name = "Middle"
path = "libs/m"
dependencies = ["Alpha"]

[[libraries]]
name = "Alpha"
path = "libs/a"
dependencies = ["Zulu"]
"#,
    );
    let issues = library_dependency_issues(&config);
    let cycles = issues_with_code(&issues, "L004");
    assert_eq!(cycles.len(), 1);
    assert!(cycles[0]
        .message
        .ends_with("Alpha -> Zulu -> Middle -> Alpha"));
}

#[test]
fn duplicate_edges_do_not_duplicate_cycle_issue() {
    let config = config(
        r#"
[[libraries]]
name = "A"
path = "libs/a"
dependencies = ["B", "b", "B"]

[[libraries]]
name = "B"
path = "libs/b"
dependencies = ["A", "a"]
"#,
    );
    assert_eq!(
        issues_with_code(&library_dependency_issues(&config), "L004").len(),
        1
    );
}

#[test]
fn distinct_cycles_each_emit_one_issue_in_canonical_order() {
    let config = config(
        r#"
[[libraries]]
name = "C"
path = "libs/c"
dependencies = ["D"]

[[libraries]]
name = "D"
path = "libs/d"
dependencies = ["C"]

[[libraries]]
name = "A"
path = "libs/a"
dependencies = ["B"]

[[libraries]]
name = "B"
path = "libs/b"
dependencies = ["A"]
"#,
    );
    let issues = library_dependency_issues(&config);
    let cycles = issues_with_code(&issues, "L004");
    assert_eq!(cycles.len(), 2);
    assert!(cycles[0].message.ends_with("A -> B -> A"));
    assert!(cycles[1].message.ends_with("C -> D -> C"));
}

#[test]
fn overlapping_cycles_remain_distinct() {
    let config = config(
        r#"
[[libraries]]
name = "A"
path = "libs/a"
dependencies = ["B", "C"]

[[libraries]]
name = "B"
path = "libs/b"
dependencies = ["A"]

[[libraries]]
name = "C"
path = "libs/c"
dependencies = ["A"]
"#,
    );
    let issues = library_dependency_issues(&config);
    let cycles = issues_with_code(&issues, "L004");
    assert_eq!(cycles.len(), 2);
    assert!(cycles
        .iter()
        .any(|issue| issue.message.ends_with("A -> B -> A")));
    assert!(cycles
        .iter()
        .any(|issue| issue.message.ends_with("A -> C -> A")));
}

#[test]
fn issue_classes_have_stable_global_order() {
    let config = config(
        r#"
[[libraries]]
name = "CycleA"
path = "libs/cycle-a"
dependencies = ["CycleB", "Missing"]

[[libraries]]
name = "CycleB"
path = "libs/cycle-b"
dependencies = ["CycleA"]

[[libraries]]
name = "Core"
path = "libs/core-v2"
version = "2"

[[libraries]]
name = "core"
path = "libs/core-v1"
version = "1"

[[libraries]]
name = "Application"
path = "libs/app"
dependencies = [{ name = "Core", version = "9" }]
"#,
    );
    let issues = library_dependency_issues(&config);
    assert_eq!(
        issues.iter().map(|issue| issue.code).collect::<Vec<_>>(),
        vec!["L003", "L001", "L002", "L004"]
    );
}

#[test]
fn version_label_is_explicit_for_absent_and_present_values() {
    assert_eq!(version_label(&None), "unspecified");
    assert_eq!(version_label(&Some("1.2.3".to_string())), "1.2.3");
}

#[test]
fn canonical_cycle_is_rotation_invariant() {
    assert_eq!(canonical_cycle(&["B", "C", "A"]), vec!["A", "B", "C", "A"]);
    assert_eq!(canonical_cycle(&["C", "A", "B"]), vec!["A", "B", "C", "A"]);
}
