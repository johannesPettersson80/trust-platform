use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{anyhow, bail, Context, Result};
use serde::{Deserialize, Serialize};

use crate::software_map::{
    CliActionSummary, DependencyEdge, DependencyHygieneSummary, DependencyPolicyEntry, DiagramEdge,
    DiagramFact, FunctionSummary, HostSurfaceSummary, ImportEdge, ModuleSummary, PackageSummary,
    ParserRecoverySummary, RuntimeRouteHandlerSummary, SafetyToolGateSummary, SoftwareMap,
    SourceFileSummary, SourcePatternSummary, TargetSummary, ToolResult, ToolStatus, UnsafeSummary,
    WorkspaceEdge,
};

pub fn architecture_doctor_full_map(root: &Path) -> Result<()> {
    let policy = FullMapPolicy::load(root)?;
    let mut map = build_software_map(root, &policy)?;
    let checks = run_policy_checks(root, &map, &policy);
    let failed = checks.iter().filter(|check| check.is_fail()).count();
    map.tool_results.push(ToolResult {
        name: "full-map policy checks".to_string(),
        status: if failed == 0 {
            ToolStatus::Pass
        } else {
            ToolStatus::Failed
        },
        details: vec![format!("failed checks: {failed}")],
    });
    let artifact_dir = full_map_artifact_dir(root)?;
    fs::create_dir_all(&artifact_dir)
        .with_context(|| format!("create {}", artifact_dir.display()))?;
    let json_path = artifact_dir.join("software-map.json");
    fs::write(&json_path, map.to_stable_json()?)
        .with_context(|| format!("write {}", json_path.display()))?;
    write_reports(&artifact_dir, &map, &checks)?;
    println!("wrote {}", json_path.display());
    println!(
        "wrote {}",
        artifact_dir.join("full-map-report.json").display()
    );
    println!(
        "wrote {}",
        artifact_dir.join("full-map-report.md").display()
    );

    for check in &checks {
        println!("{}: {}", check.status.as_str().to_uppercase(), check.id);
        for detail in &check.details {
            println!("  - {detail}");
        }
    }
    if failed > 0 {
        bail!("architecture-doctor --full-map found {failed} failing check(s)");
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct FullMapPolicy {
    policy_version: u32,
    review_date: String,
    allowed_workspace_edges: Vec<EdgePolicy>,
    forbidden_workspace_edges: Vec<EdgeKey>,
    runtime_core_forbidden_dependencies: Vec<String>,
    runtime_core_forbidden_import_modules: Vec<String>,
    runtime_command_classes: Vec<ClassifiedName>,
    runtime_bin_module_classes: Vec<ClassifiedName>,
    runtime_action_classes: Vec<ClassifiedName>,
    runtime_command_module_routes: Vec<CommandModuleRoute>,
    runtime_artifact_profiles: Vec<RuntimeArtifactProfile>,
    runtime_workbench_command_migrations: Vec<RuntimeWorkbenchCommandMigration>,
    host_surface: HostSurfacePolicy,
    kiss: KissPolicy,
    dependency_hygiene_tools: Vec<PolicyToolStatus>,
    dependency_hygiene: DependencyHygienePolicy,
    unsafe_concurrency: UnsafeConcurrencyPolicy,
    diagram_policy: DiagramPolicy,
}

#[derive(Debug, Deserialize)]
struct EdgePolicy {
    from: String,
    to: String,
    kind: String,
    status: String,
    owner: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct EdgeKey {
    from: String,
    to: String,
    kind: String,
}

#[derive(Debug, Deserialize)]
struct ClassifiedName {
    name: String,
    class: String,
    owner: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct CommandModuleRoute {
    command: String,
    module: String,
    handler: String,
    route_kind: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeArtifactProfile {
    name: String,
    class: String,
    binaries: Vec<String>,
    include_classes: Vec<String>,
    exclude_classes: Vec<String>,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeWorkbenchCommandMigration {
    command: String,
    current_binary: String,
    destination_binary: String,
    compatibility_plan: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct HostSurfacePolicy {
    approved_ports_active: bool,
    owned_paths: Vec<HostSurfaceOwnedPath>,
    forbidden_edges: Vec<ForbiddenModuleEdge>,
    temporary_allowlist: Vec<TemporaryHostSurfaceImport>,
}

#[derive(Debug, Deserialize)]
struct HostSurfaceOwnedPath {
    path_prefix: String,
    category: String,
    owner: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct ForbiddenModuleEdge {
    from_module: String,
    to_module: String,
    owner: String,
    rationale: String,
}

#[derive(Debug, Deserialize)]
struct TemporaryHostSurfaceImport {
    from_module: String,
    to_module: String,
    path: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct KissPolicy {
    new_file_line_limit: usize,
    existing_file_note_limit: usize,
    function_note_limit: usize,
    module_note_limit: usize,
    module_split_plan_line_limit: usize,
    split_plan_line_limit: usize,
    max_runtime_top_level_modules_current: usize,
    max_runtime_top_level_modules_after_boards: usize,
    enforce_after_boards_cap: bool,
    runtime_top_level_module_cap_waiver: Option<RuntimeTopLevelModuleCapWaiver>,
    runtime_top_level_module_decisions: Vec<RuntimeTopLevelModuleDecision>,
    large_file_allowlist: Vec<LargeFilePolicy>,
    module_size_allowlist: Vec<ModuleSizePolicy>,
    function_size_allowlist: Vec<FunctionSizePolicy>,
    public_api_snapshots: Vec<PublicApiSnapshotPolicy>,
}

#[derive(Debug, Deserialize)]
struct RuntimeTopLevelModuleDecision {
    name: String,
    subsystem: String,
    owner: String,
    rationale: String,
    review_date: String,
    decision_note: String,
}

#[derive(Debug, Deserialize)]
struct RuntimeTopLevelModuleCapWaiver {
    target_cap: usize,
    owner: String,
    rationale: String,
    review_date: String,
    extraction_branch: String,
    removal_condition: String,
}

#[derive(Debug, Deserialize)]
struct LargeFilePolicy {
    path: String,
    owner: String,
    rationale: String,
    review_date: String,
    split_plan: String,
}

#[derive(Debug, Deserialize)]
struct ModuleSizePolicy {
    crate_name: String,
    module_name: String,
    path: String,
    owner: String,
    rationale: String,
    review_date: String,
    split_plan: String,
}

#[derive(Debug, Deserialize)]
struct FunctionSizePolicy {
    path: String,
    name: String,
    owner: String,
    rationale: String,
    review_date: String,
    split_plan: String,
}

#[derive(Debug, Deserialize)]
struct PublicApiSnapshotPolicy {
    package: String,
    baseline: String,
    command: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct PolicyToolStatus {
    name: String,
    status: String,
    owner: String,
    rationale: String,
    review_date: String,
    evidence: String,
}

#[derive(Debug, Deserialize)]
struct DependencyHygienePolicy {
    third_party_tiverse_mmap: ThirdPartyWorkspacePolicy,
    audit_allowlist: Vec<DependencyAllowlistEntry>,
    machete_allowlist: Vec<DependencyAllowlistEntry>,
}

#[derive(Debug, Deserialize)]
struct ThirdPartyWorkspacePolicy {
    path: String,
    expected_status: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Clone, Deserialize)]
struct DependencyAllowlistEntry {
    id: String,
    package: String,
    owner: String,
    rationale: String,
    review_date: String,
    removal_condition: String,
}

#[derive(Debug, Deserialize)]
struct UnsafeConcurrencyPolicy {
    owner: String,
    status: String,
    unsafe_site_register: Vec<UnsafeSitePolicy>,
    delegated_unsafe_path_register: Vec<DelegatedUnsafePathPolicy>,
    panic_like_classifications: Vec<PanicLikeClassificationPolicy>,
    concurrency_boundaries: Vec<ConcurrencyBoundaryPolicy>,
    tool_gates: Vec<SafetyToolGatePolicy>,
}

#[derive(Debug, Deserialize)]
struct UnsafeSitePolicy {
    path: String,
    line: usize,
    owner: String,
    invariant: String,
    test_evidence: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct DelegatedUnsafePathPolicy {
    path_prefix: String,
    owner: String,
    invariant: String,
    test_evidence: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct PanicLikeClassificationPolicy {
    path_prefix: String,
    pattern: String,
    classification: String,
    owner: String,
    rationale: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct ConcurrencyBoundaryPolicy {
    path_prefix: String,
    primitive: String,
    owner: String,
    shared_state: String,
    invariant: String,
    test_evidence: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct SafetyToolGatePolicy {
    name: String,
    status: String,
    command: String,
    evidence: String,
    blocker: String,
    owner: String,
    review_date: String,
}

#[derive(Debug, Deserialize)]
struct DiagramPolicy {
    selected_diagrams: Vec<String>,
    allowed_alias_prefixes: Vec<String>,
    allowed_aliases: Vec<String>,
}

impl FullMapPolicy {
    fn load(root: &Path) -> Result<Self> {
        let path = root.join("xtask/config/full_map_policy.json");
        let source =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        serde_json::from_str(&source).with_context(|| format!("parse {}", path.display()))
    }
}

#[derive(Debug, Serialize)]
struct FullMapReport<'a> {
    status: &'a str,
    failed: usize,
    commands: Vec<&'static str>,
    tool_versions: Vec<String>,
    artifacts: Vec<String>,
    checks: &'a [FullMapCheck],
}

#[derive(Debug, Serialize)]
struct FullMapCheck {
    id: &'static str,
    status: CheckStatus,
    summary: String,
    details: Vec<String>,
}

#[derive(Debug)]
struct CommandCheckOutput {
    success: bool,
    code: Option<i32>,
    stdout: String,
    stderr: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Fail,
    Finding,
    Partial,
}

impl CheckStatus {
    fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Finding => "finding",
            Self::Partial => "partial",
        }
    }
}

impl FullMapCheck {
    fn pass(id: &'static str, summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Pass,
            summary: summary.into(),
            details,
        }
    }

    fn fail(id: &'static str, summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Fail,
            summary: summary.into(),
            details,
        }
    }

    fn finding(id: &'static str, summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Finding,
            summary: summary.into(),
            details,
        }
    }

    fn partial(id: &'static str, summary: impl Into<String>, details: Vec<String>) -> Self {
        Self {
            id,
            status: CheckStatus::Partial,
            summary: summary.into(),
            details,
        }
    }

    fn is_fail(&self) -> bool {
        self.status == CheckStatus::Fail
    }
}

fn build_software_map(root: &Path, policy: &FullMapPolicy) -> Result<SoftwareMap> {
    let _known_statuses = ToolStatus::ALL;
    let metadata = cargo_metadata(root)?;
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include workspace_members"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let packages_json = metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include packages"))?;
    let workspace_package_names = packages_json
        .iter()
        .filter(|package| {
            package["id"]
                .as_str()
                .is_some_and(|id| workspace_members.contains(id))
        })
        .filter_map(|package| package["name"].as_str())
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let mut map = SoftwareMap::new(root.display().to_string());
    for package in packages_json {
        let id = package["id"].as_str().unwrap_or_default();
        if !workspace_members.contains(id) {
            continue;
        }
        let name = package["name"].as_str().unwrap_or_default().to_string();
        let manifest_path = Path::new(package["manifest_path"].as_str().unwrap_or_default());
        map.packages.push(PackageSummary {
            name: name.clone(),
            manifest_path: rel_path(root, manifest_path),
            targets: package["targets"]
                .as_array()
                .into_iter()
                .flatten()
                .map(|target| TargetSummary {
                    name: target["name"].as_str().unwrap_or_default().to_string(),
                    kind: target["kind"]
                        .as_array()
                        .into_iter()
                        .flatten()
                        .filter_map(serde_json::Value::as_str)
                        .map(ToOwned::to_owned)
                        .collect(),
                    src_path: rel_path(
                        root,
                        Path::new(target["src_path"].as_str().unwrap_or_default()),
                    ),
                })
                .collect(),
        });
        collect_top_level_module_summaries(
            root,
            &name,
            manifest_path,
            &mut map.crate_module_summaries,
        )?;
        for dependency in package["dependencies"].as_array().into_iter().flatten() {
            let Some(dep_name) = dependency["name"].as_str() else {
                continue;
            };
            let kind = dependency["kind"].as_str().unwrap_or("normal").to_string();
            map.direct_dependencies.push(DependencyEdge {
                from: name.clone(),
                to: dep_name.to_string(),
                kind: kind.clone(),
            });
            if workspace_package_names.contains(dep_name) {
                map.workspace_edges.push(WorkspaceEdge {
                    from: name.clone(),
                    to: dep_name.to_string(),
                    kind,
                });
            }
        }
    }

    for file in collect_source_files(root)? {
        let source = fs::read_to_string(&file).unwrap_or_default();
        map.source_files.push(SourceFileSummary {
            path: rel_path(root, &file),
            line_count: source.lines().count(),
        });
    }
    map.largest_files = map.source_files.clone();
    map.largest_files.sort_by(|left, right| {
        right
            .line_count
            .cmp(&left.line_count)
            .then_with(|| left.path.cmp(&right.path))
    });
    map.largest_files.truncate(50);
    map.largest_functions = collect_runtime_function_summaries(root)?;
    map.runtime_top_level_modules = collect_runtime_top_level_modules(root)?;
    map.runtime_bin_modules = collect_runtime_bin_modules(root)?;
    let known_import_modules = map
        .runtime_top_level_modules
        .iter()
        .chain(map.runtime_bin_modules.iter())
        .cloned()
        .collect::<BTreeSet<_>>();
    map.import_edges = collect_import_edges(root, &known_import_modules)?;
    map.host_surface = collect_host_surface_summary(root)?;
    map.runtime_cli_commands = parse_enum_variants(
        &fs::read_to_string(
            root.join("crates/trust-runtime/src/bin/trust-runtime/cli/commands.rs"),
        )?,
        "Command",
    );
    map.runtime_cli_actions = collect_runtime_cli_actions(root)?;
    map.runtime_route_handlers = collect_runtime_route_handlers(root, policy)?;
    map.parser_recovery = collect_parser_recovery_summary(root);
    map.dependency_hygiene = collect_dependency_hygiene_summary(root, policy, &workspace_members)?;
    map.unsafe_summary = collect_unsafe_summary(root, policy);
    map.diagram_facts = collect_diagram_facts(root, &policy.diagram_policy)?;
    map.tool_results.push(ToolResult {
        name: "cargo metadata".to_string(),
        status: ToolStatus::Pass,
        details: vec![
            format!("workspace packages: {}", map.packages.len()),
            format!("direct dependencies: {}", map.direct_dependencies.len()),
        ],
    });
    map.tool_results.push(ToolResult {
        name: "source file scan".to_string(),
        status: ToolStatus::Pass,
        details: vec![format!("source files: {}", map.source_files.len())],
    });
    map.tool_results.push(ToolResult {
        name: "runtime CLI scan".to_string(),
        status: ToolStatus::Pass,
        details: vec![
            format!("commands: {}", map.runtime_cli_commands.len()),
            format!("action enums: {}", map.runtime_cli_actions.len()),
            format!("bin modules: {}", map.runtime_bin_modules.len()),
            format!("route handlers: {}", map.runtime_route_handlers.len()),
        ],
    });
    map.tool_results.push(ToolResult {
        name: "parser recovery scan".to_string(),
        status: if map
            .parser_recovery
            .declaration_scanner_violations
            .is_empty()
        {
            ToolStatus::Pass
        } else {
            ToolStatus::Failed
        },
        details: vec![
            format!(
                "bounded helpers: {}",
                map.parser_recovery.bounded_scan_helpers.len()
            ),
            format!(
                "scanner violations: {}",
                map.parser_recovery.declaration_scanner_violations.len()
            ),
            format!(
                "property tests: {}",
                map.parser_recovery.property_tests.len()
            ),
        ],
    });
    let public_api_version = command_version("cargo", &["public-api", "--version"]);
    map.tool_results.push(ToolResult {
        name: "cargo public-api".to_string(),
        status: if public_api_version.starts_with("cargo-public-api")
            || public_api_version.starts_with("cargo public-api")
        {
            ToolStatus::Pass
        } else {
            ToolStatus::NotRun
        },
        details: vec![
            public_api_version,
            "public API baseline enforcement is tracked by the full architecture program"
                .to_string(),
        ],
    });
    for tool in &policy.dependency_hygiene_tools {
        map.tool_results.push(ToolResult {
            name: tool.name.clone(),
            status: policy_tool_status(&tool.status),
            details: vec![
                format!("owner: {}; {}", tool.owner, tool.rationale),
                format!("review date: {}", tool.review_date),
                format!("evidence: {}", tool.evidence),
            ],
        });
    }

    Ok(map)
}

fn collect_dependency_hygiene_summary(
    root: &Path,
    policy: &FullMapPolicy,
    workspace_members: &BTreeSet<String>,
) -> Result<DependencyHygieneSummary> {
    let manifest_source = fs::read_to_string(root.join("Cargo.toml")).context("read Cargo.toml")?;
    let workspace_excludes = workspace_excludes_from_manifest_source(&manifest_source)?;
    let third_party_path = policy
        .dependency_hygiene
        .third_party_tiverse_mmap
        .path
        .as_str();
    let third_party_status = classify_workspace_path(
        root,
        workspace_members,
        &workspace_excludes,
        third_party_path,
    );

    Ok(DependencyHygieneSummary {
        deny_policy_present: root.join("deny.toml").is_file(),
        workspace_excludes,
        third_party_tiverse_mmap_status: third_party_status,
        audit_allowlist: policy
            .dependency_hygiene
            .audit_allowlist
            .iter()
            .map(dependency_policy_entry)
            .collect(),
        machete_allowlist: policy
            .dependency_hygiene
            .machete_allowlist
            .iter()
            .map(dependency_policy_entry)
            .collect(),
    })
}

fn dependency_policy_entry(entry: &DependencyAllowlistEntry) -> DependencyPolicyEntry {
    DependencyPolicyEntry {
        id: entry.id.clone(),
        package: entry.package.clone(),
        owner: entry.owner.clone(),
        rationale: entry.rationale.clone(),
        review_date: entry.review_date.clone(),
        removal_condition: entry.removal_condition.clone(),
    }
}

fn workspace_excludes_from_manifest_source(source: &str) -> Result<Vec<String>> {
    let manifest: toml::Value = toml::from_str(source).context("parse Cargo.toml as TOML")?;
    Ok(manifest
        .get("workspace")
        .and_then(|workspace| workspace.get("exclude"))
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(ToOwned::to_owned)
        .collect())
}

fn classify_workspace_path(
    root: &Path,
    workspace_members: &BTreeSet<String>,
    workspace_excludes: &[String],
    path: &str,
) -> String {
    let manifest = root.join(path).join("Cargo.toml");
    let is_member = workspace_members
        .iter()
        .any(|member| member.contains(&format!("{path}#")) || member.contains(&format!("{path}/")));
    if is_member {
        "workspace_member".to_string()
    } else if workspace_excludes.iter().any(|exclude| exclude == path) {
        "workspace_exclude".to_string()
    } else if manifest.is_file() {
        "ambiguous".to_string()
    } else {
        "missing".to_string()
    }
}

fn policy_tool_status(status: &str) -> ToolStatus {
    match status {
        "pass" => ToolStatus::Pass,
        "finding" => ToolStatus::Finding,
        "partial" => ToolStatus::Partial,
        "failed" => ToolStatus::Failed,
        _ => ToolStatus::NotRun,
    }
}

fn run_policy_checks(root: &Path, map: &SoftwareMap, policy: &FullMapPolicy) -> Vec<FullMapCheck> {
    vec![
        check_policy_metadata(policy),
        check_workspace_edge_policy(map, policy),
        check_runtime_core_dependency_fence(map, policy),
        check_runtime_command_and_module_ownership(map, policy),
        check_host_surface_edges(map, policy),
        check_dependency_hygiene_status(map, policy),
        check_unsafe_concurrency_summary(map),
        check_kiss_thresholds(map, policy),
        check_public_api_snapshot_status(root, map, policy),
        check_parser_recovery_rules(map),
        check_hir_zero_silent_bug_doctor(root),
        check_runtime_boundary_fail_closed_doctor(root),
        check_runtime_safety_fail_closed_doctor(root),
        check_runtime_vm_mutation_evidence(root),
        check_diagram_claims(map, policy),
    ]
}

fn check_policy_metadata(policy: &FullMapPolicy) -> FullMapCheck {
    let mut failures = Vec::new();
    if policy.policy_version == 0 {
        failures.push("policy_version must be non-zero".to_string());
    }
    if policy.review_date.trim().is_empty() {
        failures.push("review_date must be set".to_string());
    }
    if policy.runtime_core_forbidden_dependencies.is_empty()
        || policy.runtime_core_forbidden_import_modules.is_empty()
    {
        failures.push(
            "runtime-core forbidden dependency/import policies must be non-empty".to_string(),
        );
    }
    if policy.kiss.max_runtime_top_level_modules_current == 0
        || policy.kiss.max_runtime_top_level_modules_after_boards == 0
        || policy.kiss.function_note_limit == 0
        || policy.kiss.module_note_limit == 0
        || policy.kiss.module_split_plan_line_limit == 0
    {
        failures.push("KISS file/module/function caps must be non-zero".to_string());
    }
    for edge in &policy.allowed_workspace_edges {
        if edge.owner.trim().is_empty() || edge.rationale.trim().is_empty() {
            failures.push(format!(
                "workspace edge {} -> {} ({}) is missing owner/rationale",
                edge.from, edge.to, edge.kind
            ));
        }
        if !matches!(edge.status.as_str(), "allowed" | "temporary") {
            failures.push(format!(
                "workspace edge {} -> {} ({}) has unsupported status '{}'",
                edge.from, edge.to, edge.kind, edge.status
            ));
        }
    }
    for item in policy
        .runtime_command_classes
        .iter()
        .chain(policy.runtime_bin_module_classes.iter())
        .chain(policy.runtime_action_classes.iter())
    {
        if item.owner.trim().is_empty() || item.rationale.trim().is_empty() {
            failures.push(format!(
                "classification '{}' ({}) is missing owner/rationale",
                item.name, item.class
            ));
        }
    }
    for route in &policy.runtime_command_module_routes {
        if route.command.trim().is_empty()
            || route.module.trim().is_empty()
            || route.handler.trim().is_empty()
            || route.route_kind.trim().is_empty()
            || route.owner.trim().is_empty()
            || route.rationale.trim().is_empty()
            || route.review_date.trim().is_empty()
        {
            failures.push(format!(
                "command route Command::{} -> '{}' is missing module/handler/route_kind/owner/rationale/review_date",
                route.command, route.module
            ));
        }
    }
    for item in &policy.host_surface.temporary_allowlist {
        if item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
        {
            failures.push(format!(
                "host-surface temporary allowlist {} -> {} at '{}' is missing owner/rationale/review_date",
                item.from_module, item.to_module, item.path
            ));
        }
    }
    for item in &policy.host_surface.owned_paths {
        if item.path_prefix.trim().is_empty()
            || item.category.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
        {
            failures.push(format!(
                "host-surface owned path '{}' is missing path_prefix/category/owner/rationale",
                item.path_prefix
            ));
        }
    }
    for item in &policy.kiss.large_file_allowlist {
        if item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
            || item.split_plan.trim().is_empty()
        {
            failures.push(format!(
                "large-file allowlist entry '{}' is missing owner/rationale/review_date/split_plan",
                item.path
            ));
        }
    }
    for item in &policy.kiss.module_size_allowlist {
        if item.crate_name.trim().is_empty()
            || item.module_name.trim().is_empty()
            || item.path.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
            || item.split_plan.trim().is_empty()
        {
            failures.push(format!(
                "module-size allowlist entry '{}::{}' is missing crate/module/path/owner/rationale/review_date/split_plan",
                item.crate_name, item.module_name
            ));
        }
    }
    for item in &policy.kiss.function_size_allowlist {
        if item.path.trim().is_empty()
            || item.name.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
            || item.split_plan.trim().is_empty()
        {
            failures.push(format!(
                "function-size allowlist entry '{}::{}' is missing path/name/owner/rationale/review_date/split_plan",
                item.path, item.name
            ));
        }
    }
    for item in &policy.kiss.public_api_snapshots {
        if item.package.trim().is_empty()
            || item.baseline.trim().is_empty()
            || item.command.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
        {
            failures.push(format!(
                "public API snapshot policy for '{}' is missing package/baseline/command/owner/rationale/review_date",
                item.package
            ));
        }
    }
    if let Some(waiver) = &policy.kiss.runtime_top_level_module_cap_waiver {
        if waiver.target_cap == 0
            || waiver.owner.trim().is_empty()
            || waiver.rationale.trim().is_empty()
            || waiver.review_date.trim().is_empty()
            || waiver.extraction_branch.trim().is_empty()
            || waiver.removal_condition.trim().is_empty()
        {
            failures.push(
                "runtime top-level module cap waiver is missing target_cap/owner/rationale/review_date/extraction_branch/removal_condition"
                    .to_string(),
            );
        }
    }
    let mut runtime_module_decisions = BTreeSet::new();
    for item in &policy.kiss.runtime_top_level_module_decisions {
        if !runtime_module_decisions.insert(item.name.as_str()) {
            failures.push(format!(
                "runtime top-level module decision '{}' is duplicated",
                item.name
            ));
        }
        if item.name.trim().is_empty()
            || item.subsystem.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
            || item.decision_note.trim().is_empty()
        {
            failures.push(format!(
                "runtime top-level module decision '{}' is missing name/subsystem/owner/rationale/review_date/decision_note",
                item.name
            ));
        }
    }
    for tool in &policy.dependency_hygiene_tools {
        if tool.owner.trim().is_empty()
            || tool.rationale.trim().is_empty()
            || tool.review_date.trim().is_empty()
            || tool.evidence.trim().is_empty()
        {
            failures.push(format!(
                "dependency hygiene tool '{}' is missing owner/rationale/review_date/evidence",
                tool.name
            ));
        }
        if !matches!(
            tool.status.as_str(),
            "pass" | "finding" | "partial" | "failed" | "not_run"
        ) {
            failures.push(format!(
                "dependency hygiene tool '{}' has unsupported status '{}'",
                tool.name, tool.status
            ));
        }
    }
    let third_party = &policy.dependency_hygiene.third_party_tiverse_mmap;
    if third_party.path.trim().is_empty()
        || third_party.expected_status.trim().is_empty()
        || third_party.owner.trim().is_empty()
        || third_party.rationale.trim().is_empty()
        || third_party.review_date.trim().is_empty()
    {
        failures.push(
            "third_party/tiverse-mmap workspace policy is missing path/status/owner/rationale/review_date"
                .to_string(),
        );
    }
    failures.extend(dependency_allowlist_metadata_failures(
        "audit",
        &policy.dependency_hygiene.audit_allowlist,
    ));
    failures.extend(dependency_allowlist_metadata_failures(
        "machete",
        &policy.dependency_hygiene.machete_allowlist,
    ));
    if policy.unsafe_concurrency.owner.trim().is_empty()
        || policy.unsafe_concurrency.status.trim().is_empty()
    {
        failures.push("unsafe/concurrency policy is missing owner/status".to_string());
    }
    let mut unsafe_sites = BTreeSet::new();
    for site in &policy.unsafe_concurrency.unsafe_site_register {
        if !unsafe_sites.insert((site.path.as_str(), site.line)) {
            failures.push(format!(
                "unsafe site register entry '{}:{}' is duplicated",
                site.path, site.line
            ));
        }
        if site.path.trim().is_empty()
            || site.line == 0
            || site.owner.trim().is_empty()
            || site.invariant.trim().is_empty()
            || site.test_evidence.trim().is_empty()
            || site.review_date.trim().is_empty()
        {
            failures.push(format!(
                "unsafe site register entry '{}:{}' is missing path/line/owner/invariant/evidence/review_date",
                site.path, site.line
            ));
        }
    }
    for item in &policy.unsafe_concurrency.delegated_unsafe_path_register {
        if item.path_prefix.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.invariant.trim().is_empty()
            || item.test_evidence.trim().is_empty()
            || item.review_date.trim().is_empty()
        {
            failures.push(format!(
                "delegated unsafe path '{}' is missing path_prefix/owner/invariant/evidence/review_date",
                item.path_prefix
            ));
        }
    }
    for item in &policy.unsafe_concurrency.panic_like_classifications {
        if item.path_prefix.trim().is_empty()
            || item.pattern.trim().is_empty()
            || item.classification.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.rationale.trim().is_empty()
            || item.review_date.trim().is_empty()
        {
            failures.push(format!(
                "panic-like classification '{}' / '{}' is missing path_prefix/pattern/classification/owner/rationale/review_date",
                item.path_prefix, item.pattern
            ));
        }
    }
    for item in &policy.unsafe_concurrency.concurrency_boundaries {
        if item.path_prefix.trim().is_empty()
            || item.primitive.trim().is_empty()
            || item.owner.trim().is_empty()
            || item.shared_state.trim().is_empty()
            || item.invariant.trim().is_empty()
            || item.test_evidence.trim().is_empty()
            || item.review_date.trim().is_empty()
        {
            failures.push(format!(
                "concurrency boundary '{}' / '{}' is missing path_prefix/primitive/owner/shared_state/invariant/evidence/review_date",
                item.path_prefix, item.primitive
            ));
        }
    }
    for tool in &policy.unsafe_concurrency.tool_gates {
        if tool.name.trim().is_empty()
            || tool.status.trim().is_empty()
            || tool.command.trim().is_empty()
            || tool.owner.trim().is_empty()
            || tool.review_date.trim().is_empty()
        {
            failures.push(format!(
                "unsafe/concurrency tool gate '{}' is missing name/status/command/owner/review_date",
                tool.name
            ));
        }
        if !matches!(
            tool.status.as_str(),
            "pass" | "finding" | "partial" | "failed" | "not_run"
        ) {
            failures.push(format!(
                "unsafe/concurrency tool gate '{}' has unsupported status '{}'",
                tool.name, tool.status
            ));
        }
        if tool.status == "pass" && tool.evidence.trim().is_empty() {
            failures.push(format!(
                "unsafe/concurrency tool gate '{}' is passing without evidence",
                tool.name
            ));
        }
        if matches!(tool.status.as_str(), "partial" | "failed" | "not_run")
            && tool.blocker.trim().is_empty()
        {
            failures.push(format!(
                "unsafe/concurrency tool gate '{}' is '{}' without a blocker",
                tool.name, tool.status
            ));
        }
    }
    if failures.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-CHECK-01",
            "allowed workspace edge policy loaded with required metadata",
            vec![
                format!("policy version: {}", policy.policy_version),
                format!("review date: {}", policy.review_date),
                format!(
                    "allowed workspace edges: {}",
                    policy.allowed_workspace_edges.len()
                ),
            ],
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-CHECK-01",
            "policy metadata is incomplete",
            failures,
        )
    }
}

fn check_workspace_edge_policy(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let allowed = policy
        .allowed_workspace_edges
        .iter()
        .map(|edge| edge_tuple(&edge.from, &edge.to, &edge.kind))
        .collect::<BTreeSet<_>>();
    let forbidden = policy
        .forbidden_workspace_edges
        .iter()
        .map(|edge| edge_tuple(&edge.from, &edge.to, &edge.kind))
        .collect::<BTreeSet<_>>();

    let mut failures = Vec::new();
    for edge in &map.workspace_edges {
        let key = edge_tuple(&edge.from, &edge.to, &edge.kind);
        if forbidden.contains(&key) {
            failures.push(format!(
                "forbidden workspace edge present: {} -> {} ({})",
                edge.from, edge.to, edge.kind
            ));
        }
        if !allowed.contains(&key) {
            failures.push(format!(
                "unclassified workspace edge: {} -> {} ({})",
                edge.from, edge.to, edge.kind
            ));
        }
    }

    if failures.is_empty() {
        let temporary = policy
            .allowed_workspace_edges
            .iter()
            .filter(|edge| edge.status == "temporary")
            .map(|edge| {
                format!(
                    "{} -> {} ({}) owner={}",
                    edge.from, edge.to, edge.kind, edge.owner
                )
            })
            .collect::<Vec<_>>();
        FullMapCheck::pass(
            "FULLMAP-CHECK-02",
            "workspace edges match the allowlist and forbidden edges are absent",
            vec![
                format!("workspace edges observed: {}", map.workspace_edges.len()),
                format!("temporary policy edges: {}", temporary.len()),
            ]
            .into_iter()
            .chain(temporary)
            .collect(),
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-CHECK-02",
            "workspace edge policy rejected source-derived edges",
            failures,
        )
    }
}

fn check_runtime_core_dependency_fence(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let core_present = map
        .packages
        .iter()
        .any(|package| package.name == "trust-runtime-core");
    let core_imports_present = map
        .import_edges
        .iter()
        .any(|edge| edge.from_file.starts_with("crates/trust-runtime-core/src/"));
    if !core_present && !core_imports_present {
        return FullMapCheck::pass(
            "FULLMAP-CHECK-05",
            "trust-runtime-core dependency fence is armed; crate is not present yet",
            vec!["crate not present in cargo metadata".to_string()],
        );
    }

    let forbidden = policy
        .runtime_core_forbidden_dependencies
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let forbidden_imports = policy
        .runtime_core_forbidden_import_modules
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    let mut violations = map
        .direct_dependencies
        .iter()
        .filter(|edge| edge.from == "trust-runtime-core" && forbidden.contains(&edge.to))
        .map(|edge| {
            format!(
                "trust-runtime-core depends on forbidden crate {} ({})",
                edge.to, edge.kind
            )
        })
        .collect::<Vec<_>>();
    violations.extend(
        map.import_edges
            .iter()
            .filter(|edge| {
                edge.from_file.starts_with("crates/trust-runtime-core/src/")
                    && forbidden_imports.contains(&edge.to_module)
            })
            .map(|edge| {
                format!(
                    "trust-runtime-core imports host-only module '{}' at {}:{}",
                    edge.to_module, edge.from_file, edge.line
                )
            }),
    );

    if violations.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-CHECK-05",
            "trust-runtime-core has no forbidden direct dependencies",
            vec![
                format!("forbidden dependencies: {}", forbidden.len()),
                format!("forbidden import modules: {}", forbidden_imports.len()),
            ],
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-CHECK-05",
            "trust-runtime-core dependency fence failed",
            violations,
        )
    }
}

fn check_runtime_command_and_module_ownership(
    map: &SoftwareMap,
    policy: &FullMapPolicy,
) -> FullMapCheck {
    let command_classes = class_map(&policy.runtime_command_classes);
    let module_classes = class_map(&policy.runtime_bin_module_classes);
    let action_classes = class_map(&policy.runtime_action_classes);
    let command_routes = policy
        .runtime_command_module_routes
        .iter()
        .map(|route| (route.command.as_str(), route))
        .collect::<BTreeMap<_, _>>();
    let route_handlers = map
        .runtime_route_handlers
        .iter()
        .map(|handler| (handler.handler.as_str(), handler))
        .collect::<BTreeMap<_, _>>();
    let mut failures = Vec::new();
    let mut findings = Vec::new();
    let mut details = Vec::new();

    for command in &map.runtime_cli_commands {
        if !command_classes.contains_key(command) {
            failures.push(format!("unclassified Command variant: {command}"));
        }
    }
    for module in &map.runtime_bin_modules {
        if !module_classes.contains_key(module) {
            failures.push(format!("unclassified bin module: {module}"));
        }
    }
    for action in &map.runtime_cli_actions {
        if !action_classes.contains_key(&action.name) {
            failures.push(format!(
                "unclassified nested CLI action enum: {}",
                action.name
            ));
        }
    }

    for command in &map.runtime_cli_commands {
        let expected = command_to_module_name(command);
        if !map
            .runtime_bin_modules
            .iter()
            .any(|module| module == &expected)
        {
            if let Some(route) = command_routes.get(command.as_str()) {
                let route_target_known = map
                    .runtime_bin_modules
                    .iter()
                    .any(|module| module == &route.module)
                    || map
                        .runtime_top_level_modules
                        .iter()
                        .any(|module| module == &route.module)
                    || route.module.contains("::");
                if route_target_known {
                    if let Some(handler) = route_handlers.get(route.handler.as_str()) {
                        details.push(format!(
                            "Command::{command} has no same-name bin module '{expected}'; routes through {} handler={} at {}:{} ({}) owner={} review_date={} rationale={}",
                            route.module,
                            route.handler,
                            handler.path,
                            handler.line,
                            route.route_kind,
                            route.owner,
                            route.review_date,
                            route.rationale
                        ));
                    } else {
                        failures.push(format!(
                            "Command::{command} route handler '{}' was not found in source",
                            route.handler
                        ));
                    }
                } else {
                    failures.push(format!(
                        "Command::{command} route target '{}' is not a bin module, runtime top-level module, or explicit path",
                        route.module
                    ));
                }
            } else {
                findings.push(format!(
                    "Command::{command} has no same-name bin module '{expected}' and no runtime_command_module_routes entry"
                ));
            }
        }
    }

    for edge in &map.import_edges {
        if !is_runtime_bin_source(&edge.from_file) || is_test_source_file(&edge.from_file) {
            continue;
        }
        let Some(from_class) = module_classes.get(&edge.from_module) else {
            continue;
        };
        let Some(to_class) = module_classes.get(&edge.to_module) else {
            continue;
        };
        if productish_class(from_class) && to_class == "workbench_dev" {
            failures.push(format!(
                "product bin module '{}' imports workbench module '{}' at {}:{}",
                edge.from_module, edge.to_module, edge.from_file, edge.line
            ));
        }
    }

    validate_runtime_artifact_profiles(policy, &mut failures, &mut details);
    validate_workbench_command_migrations(&command_classes, policy, &mut failures, &mut details);

    if failures.is_empty() {
        let mut check_details = vec![
            format!(
                "Command variants classified: {}",
                map.runtime_cli_commands.len()
            ),
            format!("bin modules classified: {}", map.runtime_bin_modules.len()),
            format!(
                "nested action enums classified: {}",
                map.runtime_cli_actions.len()
            ),
        ];
        let has_findings = !findings.is_empty();
        check_details.extend(details);
        check_details.extend(findings);
        if has_findings {
            FullMapCheck::finding(
                "FULLMAP-CHECK-06",
                "runtime command/bin ownership is classified with mapping findings",
                check_details,
            )
        } else {
            FullMapCheck::pass(
                "FULLMAP-CHECK-06",
                "runtime command, nested action, and bin-module ownership is classified",
                check_details,
            )
        }
    } else {
        FullMapCheck::fail(
            "FULLMAP-CHECK-06",
            "runtime command/bin ownership policy failed",
            failures,
        )
    }
}

fn validate_runtime_artifact_profiles(
    policy: &FullMapPolicy,
    failures: &mut Vec<String>,
    details: &mut Vec<String>,
) {
    if policy.runtime_artifact_profiles.is_empty() {
        failures.push("no runtime artifact profiles are declared".to_string());
        return;
    }

    let mut has_field_runtime_profile = false;
    for profile in &policy.runtime_artifact_profiles {
        if profile.class == "field_runtime" {
            has_field_runtime_profile = true;
            if profile
                .include_classes
                .iter()
                .any(|class| class == "workbench_dev")
            {
                failures.push(format!(
                    "field runtime artifact profile '{}' includes workbench_dev",
                    profile.name
                ));
            }
            if !profile
                .exclude_classes
                .iter()
                .any(|class| class == "workbench_dev")
            {
                failures.push(format!(
                    "field runtime artifact profile '{}' does not exclude workbench_dev",
                    profile.name
                ));
            }
        }
        details.push(format!(
            "runtime artifact profile '{}' class={} binaries=[{}] include=[{}] exclude=[{}] owner={} review_date={} rationale={}",
            profile.name,
            profile.class,
            profile.binaries.join(","),
            profile.include_classes.join(","),
            profile.exclude_classes.join(","),
            profile.owner,
            profile.review_date,
            profile.rationale
        ));
    }

    if !has_field_runtime_profile {
        failures.push("no field_runtime artifact profile is declared".to_string());
    }
}

fn validate_workbench_command_migrations(
    command_classes: &BTreeMap<String, String>,
    policy: &FullMapPolicy,
    failures: &mut Vec<String>,
    details: &mut Vec<String>,
) {
    const ALLOWED_COMPATIBILITY_PLANS: &[&str] = &[
        "deprecated_forwarding_alias",
        "retained_until_destination_ships",
        "explicit_removal_after_deprecation",
    ];

    let migrations = policy
        .runtime_workbench_command_migrations
        .iter()
        .map(|migration| (migration.command.as_str(), migration))
        .collect::<BTreeMap<_, _>>();

    for (command, class) in command_classes {
        if class == "workbench_dev" && !migrations.contains_key(command.as_str()) {
            failures.push(format!(
                "workbench command '{command}' has no migration/deprecation policy"
            ));
        }
    }

    for migration in &policy.runtime_workbench_command_migrations {
        match command_classes.get(&migration.command) {
            Some(class) if class == "workbench_dev" => {}
            Some(class) => failures.push(format!(
                "migration policy for '{}' targets class '{}' instead of workbench_dev",
                migration.command, class
            )),
            None => failures.push(format!(
                "migration policy references unknown command '{}'",
                migration.command
            )),
        }
        if migration.current_binary == migration.destination_binary {
            failures.push(format!(
                "migration policy for '{}' keeps implementation in '{}'",
                migration.command, migration.current_binary
            ));
        }
        if !ALLOWED_COMPATIBILITY_PLANS
            .iter()
            .any(|plan| *plan == migration.compatibility_plan)
        {
            failures.push(format!(
                "migration policy for '{}' uses unsupported compatibility plan '{}'",
                migration.command, migration.compatibility_plan
            ));
        }
        details.push(format!(
            "workbench command '{}' migrates {} -> {} compatibility={} owner={} review_date={} rationale={}",
            migration.command,
            migration.current_binary,
            migration.destination_binary,
            migration.compatibility_plan,
            migration.owner,
            migration.review_date,
            migration.rationale
        ));
    }
}

fn check_host_surface_edges(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let mut failures = Vec::new();
    let mut details = Vec::new();
    let mut covered_files = 0usize;
    let mut categories = BTreeSet::new();
    for forbidden in &policy.host_surface.forbidden_edges {
        for edge in &map.import_edges {
            if is_test_source_file(&edge.from_file) {
                continue;
            }
            if edge.from_module == forbidden.from_module && edge.to_module == forbidden.to_module {
                if let Some(allow) = policy
                    .host_surface
                    .temporary_allowlist
                    .iter()
                    .find(|allow| {
                        allow.from_module == edge.from_module
                            && allow.to_module == edge.to_module
                            && allow.path == edge.from_file
                    })
                {
                    details.push(format!(
                        "temporary host-surface waiver {} -> {} at {}:{} owner={} review_date={} rationale={}",
                        edge.from_module,
                        edge.to_module,
                        edge.from_file,
                        edge.line,
                        allow.owner,
                        allow.review_date,
                        allow.rationale
                    ));
                    continue;
                }
                failures.push(format!(
                    "forbidden host-surface import {} -> {} at {}:{} (owner: {}; rationale: {})",
                    forbidden.from_module,
                    forbidden.to_module,
                    edge.from_file,
                    edge.line,
                    forbidden.owner,
                    forbidden.rationale
                ));
            }
        }
    }
    for file in &map.source_files {
        let path = file.path.as_str();
        if !is_host_surface_source_file(path) {
            continue;
        }
        match host_surface_owner_for_path(&policy.host_surface, path) {
            Some(owner) => {
                covered_files += 1;
                categories.insert(format!("{} ({})", owner.category, owner.owner));
            }
            None => failures.push(format!(
                "host-surface file '{}' has no owner category in policy.host_surface.owned_paths",
                path
            )),
        }
    }
    if policy.host_surface.approved_ports_active {
        for bypass in &map.host_surface.direct_runtime_state_bypasses {
            failures.push(format!(
                "web route bypasses approved host-surface port at {}:{} ({})",
                bypass.path, bypass.line, bypass.pattern
            ));
        }
        for bypass in &map.host_surface.direct_control_dispatch_bypasses {
            failures.push(format!(
                "web route bypasses approved control-dispatch port at {}:{} ({})",
                bypass.path, bypass.line, bypass.pattern
            ));
        }
    }

    if failures.is_empty() {
        details.extend(vec![
            format!(
                "approved ports active: {}",
                policy.host_surface.approved_ports_active
            ),
            format!(
                "forbidden edge rules: {}",
                policy.host_surface.forbidden_edges.len()
            ),
            format!(
                "host-surface owner path rules: {}",
                policy.host_surface.owned_paths.len()
            ),
            format!("host-surface files covered: {covered_files}"),
            format!(
                "host-surface owner categories: {}",
                categories.into_iter().collect::<Vec<_>>().join(", ")
            ),
            format!(
                "direct web runtime-state bypass findings: {}",
                map.host_surface.direct_runtime_state_bypasses.len()
            ),
            format!(
                "direct web control-dispatch bypass findings: {}",
                map.host_surface.direct_control_dispatch_bypasses.len()
            ),
        ]);
        if policy.host_surface.approved_ports_active {
            FullMapCheck::pass(
                "FULLMAP-CHECK-07",
                "host-surface ownership and forbidden direct imports are enforced",
                details,
            )
        } else {
            FullMapCheck::partial(
                "FULLMAP-CHECK-07",
                "host-surface ownership and direct-import rules are enforced; approved-port bypass rules are pending",
                details,
            )
        }
    } else {
        FullMapCheck::fail(
            "FULLMAP-CHECK-07",
            "host-surface forbidden imports were found",
            failures,
        )
    }
}

fn is_host_surface_source_file(path: &str) -> bool {
    matches!(
        path,
        "crates/trust-runtime/src/control.rs"
            | "crates/trust-runtime/src/hmi.rs"
            | "crates/trust-runtime/src/ui.rs"
            | "crates/trust-runtime/src/web.rs"
    ) || path.starts_with("crates/trust-runtime/src/control/")
        || path.starts_with("crates/trust-runtime/src/hmi/")
        || path.starts_with("crates/trust-runtime/src/runtime_cloud/")
        || path.starts_with("crates/trust-runtime/src/ui/")
        || path.starts_with("crates/trust-runtime/src/web/")
}

fn host_surface_owner_for_path<'a>(
    policy: &'a HostSurfacePolicy,
    path: &str,
) -> Option<&'a HostSurfaceOwnedPath> {
    policy
        .owned_paths
        .iter()
        .find(|entry| host_surface_path_matches(path, entry.path_prefix.as_str()))
}

fn host_surface_path_matches(path: &str, prefix: &str) -> bool {
    if prefix.ends_with('/') {
        path.starts_with(prefix)
    } else {
        path == prefix
    }
}

fn dependency_allowlist_metadata_failures(
    list_name: &str,
    entries: &[DependencyAllowlistEntry],
) -> Vec<String> {
    entries
        .iter()
        .filter(|entry| {
            entry.id.trim().is_empty()
                || entry.package.trim().is_empty()
                || entry.owner.trim().is_empty()
                || entry.rationale.trim().is_empty()
                || entry.review_date.trim().is_empty()
                || entry.removal_condition.trim().is_empty()
        })
        .map(|entry| {
            format!(
                "{list_name} dependency allowlist entry '{}' is missing id/package/owner/rationale/review_date/removal_condition",
                entry.id
            )
        })
        .collect()
}

fn check_dependency_hygiene_status(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let mut failed = Vec::new();
    let mut details = Vec::new();
    for tool in &policy.dependency_hygiene_tools {
        let detail = format!(
            "{} status={} owner={} rationale={}",
            tool.name, tool.status, tool.owner, tool.rationale
        );
        if tool.status == "failed" {
            failed.push(detail);
        } else {
            details.push(detail);
        }
    }
    let third_party = &policy.dependency_hygiene.third_party_tiverse_mmap;
    details.push(format!(
        "{} status={} expected={}",
        third_party.path,
        map.dependency_hygiene.third_party_tiverse_mmap_status,
        third_party.expected_status
    ));
    details.push(format!(
        "deny.toml present={}",
        map.dependency_hygiene.deny_policy_present
    ));
    details.push(format!(
        "audit allowlist entries={}",
        map.dependency_hygiene.audit_allowlist.len()
    ));
    details.push(format!(
        "machete allowlist entries={}",
        map.dependency_hygiene.machete_allowlist.len()
    ));
    if !map.dependency_hygiene.deny_policy_present {
        failed.push("deny.toml policy is missing".to_string());
    }
    if map.dependency_hygiene.third_party_tiverse_mmap_status != third_party.expected_status {
        failed.push(format!(
            "{} workspace status is '{}', expected '{}'",
            third_party.path,
            map.dependency_hygiene.third_party_tiverse_mmap_status,
            third_party.expected_status
        ));
    }
    if !failed.is_empty() {
        return FullMapCheck::fail(
            "FULLMAP-CHECK-08",
            "dependency hygiene policy contains failed tool status",
            failed,
        );
    }
    if policy
        .dependency_hygiene_tools
        .iter()
        .any(|tool| tool.status != "pass")
    {
        FullMapCheck::partial(
            "FULLMAP-CHECK-08",
            "dependency hygiene status is emitted but not all tools have passing evidence yet",
            details,
        )
    } else {
        FullMapCheck::pass(
            "FULLMAP-CHECK-08",
            "dependency hygiene policy status is passing",
            details,
        )
    }
}

fn check_unsafe_concurrency_summary(map: &SoftwareMap) -> FullMapCheck {
    if map.unsafe_summary.owner.trim().is_empty() || map.unsafe_summary.status.trim().is_empty() {
        return FullMapCheck::fail(
            "FULLMAP-CHECK-09",
            "unsafe/concurrency summary is missing owner/status metadata",
            Vec::new(),
        );
    }
    let mut details = vec![
        format!("owner: {}", map.unsafe_summary.owner),
        format!("status: {}", map.unsafe_summary.status),
        format!(
            "production unsafe occurrences: {}",
            map.unsafe_summary.unsafe_occurrences
        ),
        format!(
            "production panic-like occurrences: {}",
            map.unsafe_summary.panic_like_occurrences
        ),
        format!(
            "concurrency boundary occurrences: {}",
            map.unsafe_summary.concurrency_boundary_occurrences
        ),
        format!(
            "tool gates: {}",
            map.unsafe_summary
                .tool_gates
                .iter()
                .map(|tool| format!("{}={}", tool.name, tool.status))
                .collect::<Vec<_>>()
                .join(", ")
        ),
    ];
    let mut failures = Vec::new();
    if !map.unsafe_summary.unregistered_unsafe_sites.is_empty() {
        failures.push(format!(
            "unregistered unsafe sites: {}",
            map.unsafe_summary.unregistered_unsafe_sites.len()
        ));
        failures.extend(
            map.unsafe_summary
                .unregistered_unsafe_sites
                .iter()
                .take(10)
                .map(source_pattern_detail),
        );
    }
    if !map.unsafe_summary.unclassified_panic_like_sites.is_empty() {
        failures.push(format!(
            "unclassified panic-like sites: {}",
            map.unsafe_summary.unclassified_panic_like_sites.len()
        ));
        failures.extend(
            map.unsafe_summary
                .unclassified_panic_like_sites
                .iter()
                .take(10)
                .map(source_pattern_detail),
        );
    }
    if !map
        .unsafe_summary
        .unregistered_concurrency_boundaries
        .is_empty()
    {
        failures.push(format!(
            "unregistered concurrency boundaries: {}",
            map.unsafe_summary.unregistered_concurrency_boundaries.len()
        ));
        failures.extend(
            map.unsafe_summary
                .unregistered_concurrency_boundaries
                .iter()
                .take(10)
                .map(source_pattern_detail),
        );
    }
    if map.unsafe_summary.tool_gates.is_empty() {
        failures.push("unsafe/concurrency tool gates are missing".to_string());
    }
    for tool in &map.unsafe_summary.tool_gates {
        if matches!(tool.status.as_str(), "failed" | "not_run") {
            failures.push(format!(
                "unsafe/concurrency tool gate '{}' is '{}': {}",
                tool.name, tool.status, tool.blocker
            ));
        }
    }
    if !failures.is_empty() {
        return FullMapCheck::fail(
            "FULLMAP-CHECK-09",
            "unsafe/concurrency register has unowned or unclassified hotspots",
            failures,
        );
    }
    let has_partial_tools = map
        .unsafe_summary
        .tool_gates
        .iter()
        .any(|tool| matches!(tool.status.as_str(), "partial" | "finding"));
    if map.unsafe_summary.unsafe_occurrences > 0
        || map.unsafe_summary.panic_like_occurrences > 0
        || map.unsafe_summary.concurrency_boundary_occurrences > 0
        || has_partial_tools
    {
        details.push("all production hotspots are registered or classified".to_string());
        FullMapCheck::finding(
            "FULLMAP-CHECK-09",
            "unsafe/concurrency risk summary is emitted and remaining hotspots are tracked",
            details,
        )
    } else {
        FullMapCheck::pass(
            "FULLMAP-CHECK-09",
            "unsafe/concurrency hotspot summary is clean",
            details,
        )
    }
}

fn source_pattern_detail(site: &SourcePatternSummary) -> String {
    format!("{}:{}: {}", site.path, site.line, site.pattern)
}

fn is_runtime_large_file_scope(path: &str) -> bool {
    path.ends_with(".rs")
        && (path.starts_with("crates/trust-runtime/src/")
            || path.starts_with("crates/trust-runtime/tests/"))
}

fn check_kiss_thresholds(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let allowlist = policy
        .kiss
        .large_file_allowlist
        .iter()
        .map(|item| (item.path.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let runtime_file_line_counts = map
        .source_files
        .iter()
        .filter(|file| is_runtime_large_file_scope(&file.path))
        .map(|file| (file.path.as_str(), file.line_count))
        .collect::<BTreeMap<_, _>>();
    let mut failures = Vec::new();
    let mut details = Vec::new();

    for file in &map.source_files {
        if !is_runtime_large_file_scope(&file.path) {
            continue;
        }
        if file.line_count <= policy.kiss.existing_file_note_limit {
            continue;
        }
        let Some(entry) = allowlist.get(file.path.as_str()) else {
            failures.push(format!(
                "{} has {} lines and no owner/split note (threshold {})",
                file.path, file.line_count, policy.kiss.existing_file_note_limit
            ));
            continue;
        };
        details.push(format!(
            "{} lines={} owner={} split_plan={}",
            file.path, file.line_count, entry.owner, entry.split_plan
        ));
        if file.line_count > policy.kiss.split_plan_line_limit && entry.split_plan.trim().is_empty()
        {
            failures.push(format!(
                "{} has {} lines and no approved split plan (threshold {})",
                file.path, file.line_count, policy.kiss.split_plan_line_limit
            ));
        }
        if file.line_count > policy.kiss.new_file_line_limit
            && (entry.owner.trim().is_empty()
                || entry.rationale.trim().is_empty()
                || entry.review_date.trim().is_empty())
        {
            failures.push(format!(
                "{} has {} lines and incomplete KISS metadata",
                file.path, file.line_count
            ));
        }
    }
    for item in &policy.kiss.large_file_allowlist {
        if !is_runtime_large_file_scope(&item.path) {
            failures.push(format!(
                "large-file allowlist entry '{}' is outside the runtime large-file scope",
                item.path
            ));
            continue;
        }
        let Some(line_count) = runtime_file_line_counts.get(item.path.as_str()) else {
            failures.push(format!(
                "large-file allowlist entry '{}' does not match a current source file",
                item.path
            ));
            continue;
        };
        if *line_count <= policy.kiss.existing_file_note_limit {
            failures.push(format!(
                "large-file allowlist entry '{}' has {} lines, at or below threshold {}",
                item.path, line_count, policy.kiss.existing_file_note_limit
            ));
        }
    }

    let module_allowlist = policy
        .kiss
        .module_size_allowlist
        .iter()
        .map(|item| ((item.crate_name.as_str(), item.module_name.as_str()), item))
        .collect::<BTreeMap<_, _>>();
    let current_modules = map
        .crate_module_summaries
        .iter()
        .map(|module| {
            (
                (module.crate_name.as_str(), module.module_name.as_str()),
                module,
            )
        })
        .collect::<BTreeMap<_, _>>();
    let large_modules = map
        .crate_module_summaries
        .iter()
        .filter(|module| module.line_count >= policy.kiss.module_note_limit)
        .collect::<Vec<_>>();
    details.push(format!(
        "workspace top-level modules at or above {} lines: {}",
        policy.kiss.module_note_limit,
        large_modules.len()
    ));
    for module in large_modules {
        let key = (module.crate_name.as_str(), module.module_name.as_str());
        let Some(entry) = module_allowlist.get(&key) else {
            failures.push(format!(
                "{}::{} at {} has {} lines and no module-size owner/split note (threshold {})",
                module.crate_name,
                module.module_name,
                module.path,
                module.line_count,
                policy.kiss.module_note_limit
            ));
            continue;
        };
        if entry.path != module.path {
            failures.push(format!(
                "module-size allowlist entry {}::{} path '{}' does not match current path '{}'",
                entry.crate_name, entry.module_name, entry.path, module.path
            ));
        }
        details.push(format!(
            "large module {}::{} lines={} files={} owner={} split_plan={}",
            module.crate_name,
            module.module_name,
            module.line_count,
            module.file_count,
            entry.owner,
            entry.split_plan
        ));
        if module.line_count >= policy.kiss.module_split_plan_line_limit
            && entry.split_plan.trim().is_empty()
        {
            failures.push(format!(
                "{}::{} has {} lines and no approved module split plan (threshold {})",
                module.crate_name,
                module.module_name,
                module.line_count,
                policy.kiss.module_split_plan_line_limit
            ));
        }
    }
    for item in &policy.kiss.module_size_allowlist {
        let key = (item.crate_name.as_str(), item.module_name.as_str());
        let Some(module) = current_modules.get(&key) else {
            failures.push(format!(
                "module-size allowlist entry '{}::{}' does not match a current top-level module",
                item.crate_name, item.module_name
            ));
            continue;
        };
        if module.line_count < policy.kiss.module_note_limit {
            failures.push(format!(
                "module-size allowlist entry '{}::{}' has {} lines, below threshold {}",
                item.crate_name, item.module_name, module.line_count, policy.kiss.module_note_limit
            ));
        }
    }

    let runtime_module_count = map
        .runtime_top_level_modules
        .iter()
        .collect::<BTreeSet<_>>()
        .len();
    details.push(format!(
        "trust-runtime top-level modules: {runtime_module_count} (current cap {}, final host cap {})",
        policy.kiss.max_runtime_top_level_modules_current,
        policy.kiss.max_runtime_top_level_modules_after_boards
    ));
    if runtime_module_count > policy.kiss.max_runtime_top_level_modules_current {
        failures.push(format!(
            "trust-runtime top-level module count {runtime_module_count} exceeds current cap {}",
            policy.kiss.max_runtime_top_level_modules_current
        ));
    }
    if policy.kiss.enforce_after_boards_cap
        && runtime_module_count > policy.kiss.max_runtime_top_level_modules_after_boards
    {
        failures.push(format!(
            "trust-runtime top-level module count {runtime_module_count} exceeds final host cap {}",
            policy.kiss.max_runtime_top_level_modules_after_boards
        ));
    }
    if !policy.kiss.enforce_after_boards_cap {
        if runtime_module_count > policy.kiss.max_runtime_top_level_modules_after_boards {
            match &policy.kiss.runtime_top_level_module_cap_waiver {
                Some(waiver)
                    if waiver.target_cap == policy.kiss.max_runtime_top_level_modules_after_boards =>
                {
                    details.push(format!(
                        "final host cap waiver active: current={runtime_module_count} target={} owner={} branch={} removal={}",
                        waiver.target_cap,
                        waiver.owner,
                        waiver.extraction_branch,
                        waiver.removal_condition
                    ));
                }
                Some(waiver) => failures.push(format!(
                    "runtime top-level module cap waiver target {} does not match configured final cap {}",
                    waiver.target_cap, policy.kiss.max_runtime_top_level_modules_after_boards
                )),
                None => failures.push(format!(
                    "trust-runtime top-level module count {runtime_module_count} exceeds final host cap {} and no dated waiver names the next extraction branch",
                    policy.kiss.max_runtime_top_level_modules_after_boards
                )),
            }
        } else {
            details.push("final host cap is satisfied; no waiver needed".to_string());
        }
    }
    let module_decisions = policy
        .kiss
        .runtime_top_level_module_decisions
        .iter()
        .map(|item| (item.name.as_str(), item))
        .collect::<BTreeMap<_, _>>();
    let runtime_modules = map
        .runtime_top_level_modules
        .iter()
        .collect::<BTreeSet<_>>();
    for module in &runtime_modules {
        let Some(decision) = module_decisions.get(module.as_str()) else {
            failures.push(format!(
                "runtime top-level module '{module}' has no subsystem decision note"
            ));
            continue;
        };
        details.push(format!(
            "runtime module {module}: subsystem={} owner={} decision_note={}",
            decision.subsystem, decision.owner, decision.decision_note
        ));
    }
    for decision in module_decisions.keys() {
        if !runtime_modules
            .iter()
            .any(|module| module.as_str() == *decision)
        {
            failures.push(format!(
                "runtime top-level module decision '{decision}' does not match a current source module"
            ));
        }
    }
    let function_allowlist = policy
        .kiss
        .function_size_allowlist
        .iter()
        .map(|item| ((item.path.as_str(), item.name.as_str()), item))
        .collect::<BTreeMap<_, _>>();
    let current_functions = map
        .largest_functions
        .iter()
        .map(|function| ((function.path.as_str(), function.name.as_str()), function))
        .collect::<BTreeMap<_, _>>();
    let large_functions = map
        .largest_functions
        .iter()
        .filter(|function| function.line_count >= policy.kiss.function_note_limit)
        .collect::<Vec<_>>();
    details.push(format!(
        "runtime/core functions at or above {} lines: {}",
        policy.kiss.function_note_limit,
        large_functions.len()
    ));
    for function in &large_functions {
        let key = (function.path.as_str(), function.name.as_str());
        let Some(entry) = function_allowlist.get(&key) else {
            failures.push(format!(
                "{}:{} {} has {} lines and no function-size owner/split note (threshold {})",
                function.path,
                function.line,
                function.name,
                function.line_count,
                policy.kiss.function_note_limit
            ));
            continue;
        };
        details.push(format!(
            "large function {}:{} {} lines={} owner={} split_plan={}",
            function.path,
            function.line,
            function.name,
            function.line_count,
            entry.owner,
            entry.split_plan
        ));
    }
    for item in &policy.kiss.function_size_allowlist {
        let key = (item.path.as_str(), item.name.as_str());
        let Some(function) = current_functions.get(&key) else {
            failures.push(format!(
                "function-size allowlist entry '{}::{}' does not match a current runtime/core function",
                item.path, item.name
            ));
            continue;
        };
        if function.line_count < policy.kiss.function_note_limit {
            failures.push(format!(
                "function-size allowlist entry '{}::{}' has {} lines, below threshold {}",
                item.path, item.name, function.line_count, policy.kiss.function_note_limit
            ));
        }
    }

    if failures.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-CHECK-10",
            "KISS file, module, function, public API, and runtime top-level growth thresholds are enforced",
            details,
        )
    } else {
        FullMapCheck::fail("FULLMAP-CHECK-10", "KISS threshold policy failed", failures)
    }
}

fn check_public_api_snapshot_status(
    root: &Path,
    map: &SoftwareMap,
    policy: &FullMapPolicy,
) -> FullMapCheck {
    let public_api = map
        .tool_results
        .iter()
        .find(|tool| tool.name == "cargo public-api");
    let mut failures = Vec::new();
    let mut details = Vec::new();

    match public_api {
        Some(tool) if tool.status == ToolStatus::Pass => {
            details.extend(tool.details.clone());
        }
        Some(tool) => {
            details.extend(tool.details.clone());
            failures.push("cargo public-api is not available".to_string());
        }
        None => failures.push("cargo public-api tool result missing".to_string()),
    }

    if policy.kiss.public_api_snapshots.is_empty() {
        failures.push("no public API snapshots are configured".to_string());
    }
    let mut seen_packages = BTreeSet::new();
    for snapshot in &policy.kiss.public_api_snapshots {
        if !seen_packages.insert(snapshot.package.as_str()) {
            failures.push(format!(
                "public API snapshot for '{}' is duplicated",
                snapshot.package
            ));
        }
        let baseline = root.join(&snapshot.baseline);
        if !baseline.is_file() {
            failures.push(format!(
                "public API baseline for '{}' is missing at {}",
                snapshot.package, snapshot.baseline
            ));
            continue;
        }
        let line_count = fs::read_to_string(&baseline)
            .unwrap_or_default()
            .lines()
            .count();
        if line_count == 0 {
            failures.push(format!(
                "public API baseline for '{}' is empty at {}",
                snapshot.package, snapshot.baseline
            ));
        }
        details.push(format!(
            "{} baseline={} lines={} command={}",
            snapshot.package, snapshot.baseline, line_count, snapshot.command
        ));
    }

    if failures.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-P6-API",
            "public API growth baselines are configured and the snapshot tool is available",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-P6-API",
            "public API growth baseline policy failed",
            failures,
        )
    }
}

fn check_parser_recovery_rules(map: &SoftwareMap) -> FullMapCheck {
    let mut failures = Vec::new();
    let required_helpers = ["scan_top_level_ahead", "recover_top_level_until"];
    for helper in required_helpers {
        if !map
            .parser_recovery
            .bounded_scan_helpers
            .iter()
            .any(|name| name == helper)
        {
            failures.push(format!("missing bounded parser recovery helper `{helper}`"));
        }
    }
    for violation in &map.parser_recovery.declaration_scanner_violations {
        failures.push(format!(
            "{}:{} ad hoc declaration scanner pattern `{}`",
            violation.path, violation.line, violation.pattern
        ));
    }
    if map.parser_recovery.positional_diagnostic_sites.len() != 1 {
        failures.push(format!(
            "expected exactly one parser-source positional diagnostic definition, found {}",
            map.parser_recovery.positional_diagnostic_sites.len()
        ));
    }
    let required_tests = [
        "test_positional_initializer_recovery_preserves_declaration_boundaries",
        "test_initializer_recovery_property_smoke_for_generated_positional_shapes",
    ];
    for test in required_tests {
        if !map
            .parser_recovery
            .property_tests
            .iter()
            .any(|name| name == test)
        {
            failures.push(format!("missing parser recovery test `{test}`"));
        }
    }

    let mut details = vec![
        format!(
            "bounded helpers: {}",
            map.parser_recovery.bounded_scan_helpers.join(", ")
        ),
        format!(
            "positional diagnostic source definitions: {}",
            map.parser_recovery.positional_diagnostic_sites.len()
        ),
        format!(
            "parser recovery tests: {}",
            map.parser_recovery.property_tests.join(", ")
        ),
    ];
    details.extend(
        map.parser_recovery
            .declaration_scanner_violations
            .iter()
            .map(|violation| {
                format!(
                    "violation {}:{} {}",
                    violation.path, violation.line, violation.pattern
                )
            }),
    );

    if failures.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-PARSERREC",
            "parser recovery uses bounded helpers and locked tests",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-PARSERREC",
            "parser recovery guardrail failed",
            failures,
        )
    }
}

fn check_hir_zero_silent_bug_doctor(root: &Path) -> FullMapCheck {
    let script = Path::new("scripts/hir_zero_silent_bug_doctor.py");
    let script_path = root.join(script);
    let command = "python3 scripts/hir_zero_silent_bug_doctor.py --fail";
    if !script_path.is_file() {
        return FullMapCheck::fail(
            "FULLMAP-HIRZSB",
            "HIR zero-silent-bug doctor script is missing",
            vec![format!("missing {}", script.display())],
        );
    }

    match Command::new("python3")
        .arg(script)
        .arg("--fail")
        .current_dir(root)
        .output()
    {
        Ok(output) => hir_zero_silent_bug_doctor_check_from_output(
            command,
            CommandCheckOutput {
                success: output.status.success(),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
        ),
        Err(error) => FullMapCheck::fail(
            "FULLMAP-HIRZSB",
            "HIR zero-silent-bug doctor could not run",
            vec![format!("command: {command}"), format!("error: {error}")],
        ),
    }
}

fn hir_zero_silent_bug_doctor_check_from_output(
    command: &str,
    output: CommandCheckOutput,
) -> FullMapCheck {
    let exit_code = output.code.map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let mut details = vec![
        format!("command: {command}"),
        format!("exit code: {exit_code}"),
    ];
    details.extend(command_stream_details("stdout", &output.stdout));
    details.extend(command_stream_details("stderr", &output.stderr));

    if output.success
        && output
            .stdout
            .contains("HIR zero-silent-bug doctor: no findings")
    {
        FullMapCheck::pass(
            "FULLMAP-HIRZSB",
            "HIR zero-silent-bug doctor reported no findings",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-HIRZSB",
            "HIR zero-silent-bug doctor reported findings or failed",
            details,
        )
    }
}

fn check_runtime_boundary_fail_closed_doctor(root: &Path) -> FullMapCheck {
    let script = Path::new("scripts/runtime_boundary_fail_closed_ast_grep_gate.sh");
    let script_path = root.join(script);
    let command = "./scripts/runtime_boundary_fail_closed_ast_grep_gate.sh";
    if !script_path.is_file() {
        return FullMapCheck::fail(
            "FULLMAP-RUNTIMEBOUND",
            "runtime boundary fail-closed gate script is missing",
            vec![format!("missing {}", script.display())],
        );
    }

    match Command::new(script).current_dir(root).output() {
        Ok(output) => runtime_boundary_fail_closed_check_from_output(
            command,
            CommandCheckOutput {
                success: output.status.success(),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
        ),
        Err(error) => FullMapCheck::fail(
            "FULLMAP-RUNTIMEBOUND",
            "runtime boundary fail-closed gate could not run",
            vec![format!("command: {command}"), format!("error: {error}")],
        ),
    }
}

fn runtime_boundary_fail_closed_check_from_output(
    command: &str,
    output: CommandCheckOutput,
) -> FullMapCheck {
    let exit_code = output.code.map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let mut details = vec![
        format!("command: {command}"),
        format!("exit code: {exit_code}"),
    ];
    details.extend(command_stream_details("stdout", &output.stdout));
    details.extend(command_stream_details("stderr", &output.stderr));

    if output.success
        && output
            .stdout
            .contains("runtime boundary fail-closed gate: no findings")
    {
        FullMapCheck::pass(
            "FULLMAP-RUNTIMEBOUND",
            "runtime boundary fail-closed gate reported no findings",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-RUNTIMEBOUND",
            "runtime boundary fail-closed gate reported findings or failed",
            details,
        )
    }
}

fn check_runtime_safety_fail_closed_doctor(root: &Path) -> FullMapCheck {
    let script = Path::new("scripts/runtime_safety_fail_closed_ast_grep_gate.sh");
    let script_path = root.join(script);
    let command = "./scripts/runtime_safety_fail_closed_ast_grep_gate.sh";
    if !script_path.is_file() {
        return FullMapCheck::fail(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate script is missing",
            vec![format!("missing {}", script.display())],
        );
    }

    match Command::new(script).current_dir(root).output() {
        Ok(output) => runtime_safety_fail_closed_check_from_output(
            command,
            CommandCheckOutput {
                success: output.status.success(),
                code: output.status.code(),
                stdout: String::from_utf8_lossy(&output.stdout).into_owned(),
                stderr: String::from_utf8_lossy(&output.stderr).into_owned(),
            },
        ),
        Err(error) => FullMapCheck::fail(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate could not run",
            vec![format!("command: {command}"), format!("error: {error}")],
        ),
    }
}

fn runtime_safety_fail_closed_check_from_output(
    command: &str,
    output: CommandCheckOutput,
) -> FullMapCheck {
    let exit_code = output.code.map_or_else(
        || "terminated by signal".to_string(),
        |code| code.to_string(),
    );
    let mut details = vec![
        format!("command: {command}"),
        format!("exit code: {exit_code}"),
    ];
    details.extend(command_stream_details("stdout", &output.stdout));
    details.extend(command_stream_details("stderr", &output.stderr));

    if !output.success {
        return FullMapCheck::fail(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate failed to run",
            details,
        );
    }

    if output
        .stdout
        .contains("runtime safety fail-closed gate: no findings")
    {
        FullMapCheck::pass(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate reported no findings",
            details,
        )
    } else if output
        .stdout
        .contains("runtime safety fail-closed gate: findings")
    {
        FullMapCheck::fail(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate reported findings",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-RUNTIMESAFE",
            "runtime safety fail-closed gate returned unrecognized output",
            details,
        )
    }
}

#[derive(Debug, Clone)]
struct RuntimeVmMutationShardEvidence {
    shard: String,
    total: u64,
    caught: u64,
    missed: u64,
    timeout: u64,
    unviable: u64,
    outcomes_path: String,
}

fn check_runtime_vm_mutation_evidence(root: &Path) -> FullMapCheck {
    let mut evidence = Vec::new();
    let mut missing = Vec::new();
    let mut errors = Vec::new();
    let out_root = root.join("target/gate-artifacts/runtime-vm-mutants");
    for shard in RUNTIME_VM_MUTATION_SHARDS {
        match read_runtime_vm_mutation_shard_evidence(root, &out_root, shard) {
            Ok(Some(item)) => evidence.push(item),
            Ok(None) => missing.push((*shard).to_string()),
            Err(error) => errors.push(error),
        }
    }
    runtime_vm_mutation_evidence_check_from_parts(evidence, missing, errors)
}

fn read_runtime_vm_mutation_shard_evidence(
    root: &Path,
    out_root: &Path,
    shard: &str,
) -> Result<Option<RuntimeVmMutationShardEvidence>, String> {
    let outcomes_path = out_root.join(shard).join("mutants.out/outcomes.json");
    if !outcomes_path.is_file() {
        return Ok(None);
    }
    let source = fs::read_to_string(&outcomes_path)
        .map_err(|error| format!("read {}: {error}", rel_path(root, &outcomes_path)))?;
    let json = serde_json::from_str::<serde_json::Value>(&source)
        .map_err(|error| format!("parse {}: {error}", rel_path(root, &outcomes_path)))?;
    let count = |name: &str| -> Result<u64, String> {
        json.get(name)
            .and_then(serde_json::Value::as_u64)
            .ok_or_else(|| {
                format!(
                    "{} is missing numeric field '{name}'",
                    rel_path(root, &outcomes_path)
                )
            })
    };
    Ok(Some(RuntimeVmMutationShardEvidence {
        shard: shard.to_string(),
        total: count("total_mutants")?,
        caught: count("caught")?,
        missed: count("missed")?,
        timeout: count("timeout")?,
        unviable: count("unviable")?,
        outcomes_path: rel_path(root, &outcomes_path),
    }))
}

fn runtime_vm_mutation_evidence_check_from_parts(
    evidence: Vec<RuntimeVmMutationShardEvidence>,
    missing: Vec<String>,
    errors: Vec<String>,
) -> FullMapCheck {
    let mut details = Vec::new();
    details.push(format!(
        "expected shards: {}",
        RUNTIME_VM_MUTATION_SHARDS.len()
    ));
    details.push(format!("present shards: {}", evidence.len()));
    for item in &evidence {
        details.push(format!(
            "{}: {} total / {} caught / {} unviable / {} missed / {} timeout ({})",
            item.shard,
            item.total,
            item.caught,
            item.unviable,
            item.missed,
            item.timeout,
            item.outcomes_path
        ));
    }
    for shard in &missing {
        details.push(format!(
            "{shard}: missing target/gate-artifacts/runtime-vm-mutants/{shard}/mutants.out/outcomes.json"
        ));
    }
    details.extend(errors.iter().map(|error| format!("error: {error}")));

    let mut failures = errors;
    failures.extend(evidence.iter().filter_map(|item| {
        if item.missed > 0 || item.timeout > 0 {
            Some(format!(
                "{} has {} missed and {} timeout mutants",
                item.shard, item.missed, item.timeout
            ))
        } else {
            None
        }
    }));

    if !failures.is_empty() {
        return FullMapCheck::fail(
            "FULLMAP-RUNTIMEVM-MUT",
            "runtime VM mutation evidence has unexplained survivors",
            failures,
        );
    }
    if !missing.is_empty() {
        return FullMapCheck::partial(
            "FULLMAP-RUNTIMEVM-MUT",
            "runtime VM mutation evidence artifacts are incomplete",
            details,
        );
    }
    FullMapCheck::pass(
        "FULLMAP-RUNTIMEVM-MUT",
        "runtime VM mutation shards have zero missed and timeout mutants",
        details,
    )
}

const RUNTIME_VM_MUTATION_SHARDS: &[&str] = &[
    "call-root",
    "call-bindings",
    "call-stdlib",
    "call-symbols",
    "register-ir-root",
    "register-ir-interpreter",
    "register-ir-lower-root",
    "register-ir-lower-decode",
    "register-ir-lower-fuse",
    "register-ir-lower-verify",
    "register-ir-tier1-root",
    "register-ir-tier1-compile",
    "register-ir-tier1-execute",
    "register-ir-tier1-state",
];

fn command_stream_details(label: &str, stream: &str) -> Vec<String> {
    let mut details = stream
        .lines()
        .filter(|line| !line.trim().is_empty())
        .take(20)
        .map(|line| format!("{label}: {line}"))
        .collect::<Vec<_>>();
    if stream
        .lines()
        .filter(|line| !line.trim().is_empty())
        .count()
        > 20
    {
        details.push(format!("{label}: ... truncated"));
    }
    details
}

fn check_diagram_claims(map: &SoftwareMap, policy: &FullMapPolicy) -> FullMapCheck {
    let mut failures = Vec::new();
    let mut details = Vec::new();
    let selected = policy
        .diagram_policy
        .selected_diagrams
        .iter()
        .collect::<BTreeSet<_>>();
    let workspace_edges = map
        .workspace_edges
        .iter()
        .map(|edge| (crate_alias(&edge.from), crate_alias(&edge.to)))
        .collect::<BTreeSet<_>>();

    for diagram in &map.diagram_facts {
        if !selected.contains(&diagram.path) {
            failures.push(format!(
                "diagram fact emitted for unselected diagram {}",
                diagram.path
            ));
        }
        for component in &diagram.components {
            if !diagram_alias_allowed(component, &policy.diagram_policy, map) {
                failures.push(format!(
                    "{} contains unsupported component alias '{}'",
                    diagram.path, component
                ));
            }
        }
        let components = diagram.components.iter().collect::<BTreeSet<_>>();
        for edge in &diagram.edges {
            if !components.contains(&edge.from) {
                failures.push(format!(
                    "{} edge starts at undeclared alias '{}'",
                    diagram.path, edge.from
                ));
            }
            if !components.contains(&edge.to) {
                failures.push(format!(
                    "{} edge points at undeclared alias '{}'",
                    diagram.path, edge.to
                ));
            }
            if edge.from.starts_with("crate_")
                && edge.to.starts_with("crate_")
                && !workspace_edges.contains(&(edge.from.clone(), edge.to.clone()))
            {
                failures.push(format!(
                    "{} has unsupported crate dependency claim {} -> {}",
                    diagram.path, edge.from, edge.to
                ));
            }
        }
        details.push(format!(
            "{} components={} edges={}",
            diagram.path,
            diagram.components.len(),
            diagram.edges.len()
        ));
    }

    if failures.is_empty() {
        FullMapCheck::pass(
            "FULLMAP-P7",
            "selected diagram component aliases and crate dependency claims match map facts",
            details,
        )
    } else {
        FullMapCheck::fail(
            "FULLMAP-P7",
            "diagram semantic claim check failed",
            failures,
        )
    }
}

fn write_reports(artifact_dir: &Path, map: &SoftwareMap, checks: &[FullMapCheck]) -> Result<()> {
    let failed = checks.iter().filter(|check| check.is_fail()).count();
    let status = if failed == 0 { "pass" } else { "fail" };
    let artifacts = vec![
        "software-map.json".to_string(),
        "full-map-report.json".to_string(),
        "full-map-report.md".to_string(),
    ];
    let tool_versions = vec![
        command_version("cargo", &["--version"]),
        command_version("rustc", &["--version"]),
    ];
    let report = FullMapReport {
        status,
        failed,
        commands: vec!["cargo xtask architecture-doctor --full-map"],
        tool_versions: tool_versions.clone(),
        artifacts: artifacts.clone(),
        checks,
    };
    let report_json = serde_json::to_string_pretty(&report)?;
    fs::write(artifact_dir.join("full-map-report.json"), report_json)?;

    let mut markdown = String::new();
    markdown.push_str("# Full-Map Architecture Doctor Report\n\n");
    markdown.push_str(&format!("Status: `{status}`\n\n"));
    markdown.push_str("## Command\n\n");
    markdown.push_str("- `cargo xtask architecture-doctor --full-map`\n\n");
    markdown.push_str("## Tool Versions\n\n");
    for version in &tool_versions {
        markdown.push_str(&format!("- {version}\n"));
    }
    markdown.push_str("\n## Artifacts\n\n");
    for artifact in &artifacts {
        markdown.push_str(&format!("- `{artifact}`\n"));
    }
    markdown.push_str("\n## Source Facts\n\n");
    markdown.push_str(&format!("- Packages: {}\n", map.packages.len()));
    markdown.push_str(&format!(
        "- Workspace edges: {}\n",
        map.workspace_edges.len()
    ));
    markdown.push_str(&format!("- Source files: {}\n", map.source_files.len()));
    markdown.push_str(&format!("- Import edges: {}\n", map.import_edges.len()));
    markdown.push_str(&format!(
        "- Runtime top-level modules: {}\n",
        map.runtime_top_level_modules
            .iter()
            .collect::<BTreeSet<_>>()
            .len()
    ));
    markdown.push_str("\n## Checks\n\n");
    for check in checks {
        markdown.push_str(&format!(
            "### {} - {}\n\n{}\n\n",
            check.status.as_str().to_uppercase(),
            check.id,
            check.summary
        ));
        for detail in &check.details {
            markdown.push_str(&format!("- {detail}\n"));
        }
        markdown.push('\n');
    }
    fs::write(artifact_dir.join("full-map-report.md"), markdown)?;
    Ok(())
}

fn collect_top_level_module_summaries(
    root: &Path,
    crate_name: &str,
    manifest_path: &Path,
    summaries: &mut Vec<ModuleSummary>,
) -> Result<()> {
    let Some(crate_dir) = manifest_path.parent() else {
        return Ok(());
    };
    let src_dir = crate_dir.join("src");
    if !src_dir.exists() {
        return Ok(());
    }
    let mut by_module = BTreeMap::<String, (PathBuf, usize, usize)>::new();
    for entry in fs::read_dir(&src_dir).with_context(|| format!("read {}", src_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let module_name = entry.file_name().to_string_lossy().to_string();
            let (files, lines) = count_rs_files_and_lines(&path)?;
            by_module
                .entry(module_name)
                .and_modify(|existing| {
                    existing.0 = path.clone();
                    existing.1 += files;
                    existing.2 += lines;
                })
                .or_insert((path, files, lines));
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            let line_count = fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .count();
            by_module
                .entry(stem.to_string())
                .and_modify(|existing| {
                    existing.1 += 1;
                    existing.2 += line_count;
                })
                .or_insert((path, 1, line_count));
        }
    }
    for (module_name, (path, file_count, line_count)) in by_module {
        summaries.push(ModuleSummary {
            crate_name: crate_name.to_string(),
            module_name,
            path: rel_path(root, &path),
            file_count,
            line_count,
        });
    }
    Ok(())
}

fn count_rs_files_and_lines(path: &Path) -> Result<(usize, usize)> {
    let mut file_count = 0;
    let mut line_count = 0;
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            let (nested_files, nested_lines) = count_rs_files_and_lines(&path)?;
            file_count += nested_files;
            line_count += nested_lines;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            file_count += 1;
            line_count += fs::read_to_string(&path)
                .unwrap_or_default()
                .lines()
                .count();
        }
    }
    Ok((file_count, line_count))
}

fn collect_runtime_function_summaries(root: &Path) -> Result<Vec<FunctionSummary>> {
    let mut functions = Vec::new();
    for rel in ["crates/trust-runtime/src", "crates/trust-runtime-core/src"] {
        let path = root.join(rel);
        if !path.exists() {
            continue;
        }
        let mut files = Vec::new();
        collect_source_files_inner(&path, &mut files)?;
        for file in files {
            if file.extension().and_then(|ext| ext.to_str()) != Some("rs") {
                continue;
            }
            let source = fs::read_to_string(&file).unwrap_or_default();
            functions.extend(function_summaries_from_source(
                &rel_path(root, &file),
                &source,
            ));
        }
    }
    functions.sort_by(|left, right| {
        right
            .line_count
            .cmp(&left.line_count)
            .then_with(|| left.path.cmp(&right.path))
            .then_with(|| left.line.cmp(&right.line))
            .then_with(|| left.name.cmp(&right.name))
    });
    Ok(functions)
}

fn function_summaries_from_source(path: &str, source: &str) -> Vec<FunctionSummary> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut functions = Vec::new();
    let mut index = 0usize;
    while index < lines.len() {
        let line = lines[index];
        let Some(name) = function_name_from_line(line) else {
            index += 1;
            continue;
        };
        let start = index;
        let mut cursor = index;
        let mut saw_body = false;
        let mut brace_depth = 0isize;
        while cursor < lines.len() {
            let body_line = strip_line_comment(lines[cursor]);
            for ch in body_line.chars() {
                match ch {
                    '{' => {
                        saw_body = true;
                        brace_depth += 1;
                    }
                    '}' if saw_body => {
                        brace_depth -= 1;
                    }
                    _ => {}
                }
            }
            if saw_body && brace_depth <= 0 {
                break;
            }
            cursor += 1;
        }
        if saw_body {
            functions.push(FunctionSummary {
                path: path.to_string(),
                line: start + 1,
                name,
                line_count: cursor.saturating_sub(start) + 1,
            });
            index = cursor.saturating_add(1);
        } else {
            index += 1;
        }
    }
    functions
}

fn function_name_from_line(line: &str) -> Option<String> {
    let trimmed = line.trim_start();
    if trimmed.starts_with("//") {
        return None;
    }
    let position = trimmed.find("fn ")?;
    if position > 0 {
        let before = &trimmed[..position];
        let valid_prefix = before
            .split_whitespace()
            .all(|token| matches!(token, "pub" | "async" | "const" | "unsafe" | "extern"));
        if !valid_prefix && !before.contains("pub(") {
            return None;
        }
    }
    let rest = &trimmed[position + 3..];
    let name = rest
        .chars()
        .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
        .collect::<String>();
    (!name.is_empty()).then_some(name)
}

fn collect_import_edges(root: &Path, known_modules: &BTreeSet<String>) -> Result<Vec<ImportEdge>> {
    let mut edges = Vec::new();
    let runtime_src = root.join("crates/trust-runtime/src");
    let bin_root = runtime_src.join("bin/trust-runtime");
    let mut files = Vec::new();
    collect_source_files_inner(&runtime_src, &mut files)?;
    for file in files {
        if file.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let from_module = runtime_module_for_path(&runtime_src, &bin_root, &file);
        let source = fs::read_to_string(&file).unwrap_or_default();
        for (idx, line) in source.lines().enumerate() {
            let line = strip_line_comment(line);
            for module in source_line_modules(line) {
                if !known_modules.contains(&module) {
                    continue;
                }
                if module != from_module {
                    edges.push(ImportEdge {
                        from_file: rel_path(root, &file),
                        from_module: from_module.clone(),
                        to_module: module,
                        line: idx + 1,
                    });
                }
            }
        }
    }
    Ok(edges)
}

fn collect_host_surface_summary(root: &Path) -> Result<HostSurfaceSummary> {
    let web_root = root.join("crates/trust-runtime/src/web");
    let mut files = Vec::new();
    collect_source_files_inner(&web_root, &mut files)?;
    let mut summary = HostSurfaceSummary::default();
    for file in files {
        if file.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let rel = rel_path(root, &file);
        if is_test_source_file(&rel) {
            continue;
        }
        let source = fs::read_to_string(&file).unwrap_or_default();
        for (idx, line) in source.lines().enumerate() {
            let line = strip_line_comment(line);
            if let Some(field) = direct_control_state_field_bypass(line) {
                summary
                    .direct_runtime_state_bypasses
                    .push(SourcePatternSummary {
                        path: rel.clone(),
                        line: idx + 1,
                        pattern: format!("ControlState.{field} direct web access"),
                    });
            }
            if direct_control_dispatch_bypass(line) {
                summary
                    .direct_control_dispatch_bypasses
                    .push(SourcePatternSummary {
                        path: rel.clone(),
                        line: idx + 1,
                        pattern: "handle_request_value direct web dispatch".to_string(),
                    });
            }
        }
    }
    Ok(summary)
}

fn direct_control_dispatch_bypass(line: &str) -> bool {
    line.contains("handle_request_value(")
        || line.contains("use crate::control::handle_request_value")
}

fn direct_control_state_field_bypass(line: &str) -> Option<&'static str> {
    const FORBIDDEN_FIELDS: &[&str] = &[
        "debug",
        "resource",
        "metadata",
        "sources",
        "io_snapshot",
        "pending_restart",
        "metrics",
        "events",
        "settings",
        "realtime_status",
        "project_root",
        "resource_name",
        "io_health",
        "debug_variables",
        "hmi_live",
        "hmi_descriptor",
        "historian",
    ];
    for field in FORBIDDEN_FIELDS {
        let direct = format!("control_state.{field}");
        let context = format!("control_state.as_ref().{field}");
        if line.contains(&direct) || line.contains(&context) {
            return Some(field);
        }
    }
    None
}

fn collect_unsafe_summary(root: &Path, policy: &FullMapPolicy) -> UnsafeSummary {
    const PANIC_LIKE_NEEDLES: [&str; 5] =
        ["unwrap(", "expect(", "panic!", "todo!", "unimplemented!"];
    const CONCURRENCY_NEEDLES: [&str; 18] = [
        "thread::spawn",
        "std::thread",
        "tokio::spawn",
        "spawn_blocking",
        "JoinHandle",
        "mpsc",
        "channel(",
        "Mutex",
        "RwLock",
        "Arc<",
        "Atomic",
        "Ordering::",
        "shared_memory",
        "SharedMemory",
        "WebSocket",
        "tungstenite",
        "parking_lot",
        "Condvar",
    ];

    let mut production_unsafe_sites = Vec::new();
    let mut production_panic_like_sites = Vec::new();
    let mut concurrency_boundary_sites = Vec::new();

    if let Ok(files) = collect_safety_scan_files(root) {
        for file in files {
            let rel = rel_path(root, &file);
            if is_test_like_source_path(&rel) {
                continue;
            }
            let source = fs::read_to_string(&file).unwrap_or_default();
            for (idx, line, production_line) in production_source_lines(&source) {
                if !production_line {
                    continue;
                }
                let line_without_comment = strip_line_comment(line);
                if line_without_comment.contains("unsafe")
                    && !line_without_comment.contains("forbid(unsafe_code)")
                    && !line_without_comment.contains("deny(unsafe_code)")
                {
                    production_unsafe_sites.push(SourcePatternSummary {
                        path: rel.clone(),
                        line: idx,
                        pattern: line.trim().to_string(),
                    });
                }
                if is_panic_like_scan_path(&rel)
                    && PANIC_LIKE_NEEDLES
                        .iter()
                        .any(|needle| line_without_comment.contains(needle))
                {
                    production_panic_like_sites.push(SourcePatternSummary {
                        path: rel.clone(),
                        line: idx,
                        pattern: line.trim().to_string(),
                    });
                }
                if is_concurrency_scan_path(&rel)
                    && CONCURRENCY_NEEDLES
                        .iter()
                        .any(|needle| line_without_comment.contains(needle))
                {
                    concurrency_boundary_sites.push(SourcePatternSummary {
                        path: rel.clone(),
                        line: idx,
                        pattern: line.trim().to_string(),
                    });
                }
            }
        }
    }

    let unregistered_unsafe_sites = production_unsafe_sites
        .iter()
        .filter(|site| !unsafe_site_is_registered(site, &policy.unsafe_concurrency))
        .cloned()
        .collect::<Vec<_>>();
    let unclassified_panic_like_sites = production_panic_like_sites
        .iter()
        .filter(|site| !panic_like_site_is_classified(site, &policy.unsafe_concurrency))
        .cloned()
        .collect::<Vec<_>>();
    let unregistered_concurrency_boundaries = concurrency_boundary_sites
        .iter()
        .filter(|site| !concurrency_boundary_is_registered(site, &policy.unsafe_concurrency))
        .cloned()
        .collect::<Vec<_>>();

    let tool_gates = policy
        .unsafe_concurrency
        .tool_gates
        .iter()
        .map(|tool| SafetyToolGateSummary {
            name: tool.name.clone(),
            status: tool.status.clone(),
            command: tool.command.clone(),
            evidence: tool.evidence.clone(),
            blocker: tool.blocker.clone(),
        })
        .collect();

    UnsafeSummary {
        unsafe_occurrences: production_unsafe_sites.len(),
        panic_like_occurrences: production_panic_like_sites.len(),
        concurrency_boundary_occurrences: concurrency_boundary_sites.len(),
        owner: policy.unsafe_concurrency.owner.clone(),
        status: policy.unsafe_concurrency.status.clone(),
        production_unsafe_sites,
        production_panic_like_sites,
        concurrency_boundary_sites,
        unregistered_unsafe_sites,
        unclassified_panic_like_sites,
        unregistered_concurrency_boundaries,
        tool_gates,
    }
}

fn collect_safety_scan_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for rel in ["crates", "third_party"] {
        let dir = root.join(rel);
        if dir.exists() {
            collect_safety_scan_files_inner(&dir, &mut files)?;
        }
    }
    Ok(files)
}

fn collect_safety_scan_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect_safety_scan_files_inner(&path, files)?;
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            files.push(path);
        }
    }
    Ok(())
}

fn is_test_like_source_path(path: &str) -> bool {
    path.contains("/tests/")
        || path.contains("/test/")
        || path.contains("_tests")
        || path.ends_with("tests.rs")
        || path.ends_with("_test.rs")
}

fn is_panic_like_scan_path(path: &str) -> bool {
    path.starts_with("crates/trust-runtime/src/")
        || path.starts_with("crates/trust-hir/src/")
        || path.starts_with("crates/trust-lsp/src/")
        || path.starts_with("crates/trust-ide/src/")
}

fn is_concurrency_scan_path(path: &str) -> bool {
    is_panic_like_scan_path(path)
}

fn production_source_lines(source: &str) -> Vec<(usize, &str, bool)> {
    let mut result = Vec::new();
    let mut pending_cfg_test = false;
    let mut cfg_test_depth: Option<isize> = None;

    for (idx, line) in source.lines().enumerate() {
        let line_no = idx + 1;
        let trimmed = line.trim();
        if let Some(depth) = cfg_test_depth.as_mut() {
            *depth += count_char(line, '{') as isize;
            *depth -= count_char(line, '}') as isize;
            result.push((line_no, line, false));
            if *depth <= 0 {
                cfg_test_depth = None;
            }
            continue;
        }
        if trimmed.starts_with("#[cfg(test)]") {
            pending_cfg_test = true;
            result.push((line_no, line, false));
            continue;
        }
        if pending_cfg_test
            && (trimmed.starts_with("mod tests") || trimmed.starts_with("pub mod tests"))
        {
            let mut depth = count_char(line, '{') as isize - count_char(line, '}') as isize;
            if depth <= 0 {
                depth = 1;
            }
            cfg_test_depth = Some(depth);
            pending_cfg_test = false;
            result.push((line_no, line, false));
            continue;
        }
        if pending_cfg_test && !trimmed.is_empty() && !trimmed.starts_with("#[") {
            pending_cfg_test = false;
        }
        result.push((line_no, line, true));
    }

    result
}

fn unsafe_site_is_registered(
    site: &SourcePatternSummary,
    policy: &UnsafeConcurrencyPolicy,
) -> bool {
    policy
        .unsafe_site_register
        .iter()
        .any(|entry| entry.path == site.path && entry.line == site.line)
        || policy
            .delegated_unsafe_path_register
            .iter()
            .any(|entry| site.path.starts_with(&entry.path_prefix))
}

fn panic_like_site_is_classified(
    site: &SourcePatternSummary,
    policy: &UnsafeConcurrencyPolicy,
) -> bool {
    policy.panic_like_classifications.iter().any(|entry| {
        site.path.starts_with(&entry.path_prefix)
            && (entry.pattern == "*" || site.pattern.contains(&entry.pattern))
    })
}

fn concurrency_boundary_is_registered(
    site: &SourcePatternSummary,
    policy: &UnsafeConcurrencyPolicy,
) -> bool {
    policy.concurrency_boundaries.iter().any(|entry| {
        site.path.starts_with(&entry.path_prefix)
            && (entry.primitive == "*" || site.pattern.contains(&entry.primitive))
    })
}

fn collect_parser_recovery_summary(root: &Path) -> ParserRecoverySummary {
    let mut summary = ParserRecoverySummary::default();
    let parser_path = root.join("crates/trust-syntax/src/parser/parser.rs");
    let declarations_path = root.join("crates/trust-syntax/src/parser/grammar/declarations.rs");
    let parser_variables_path = root.join("crates/trust-syntax/tests/parser_variables.rs");

    if let Ok(source) = fs::read_to_string(&parser_path) {
        for line in source.lines() {
            if line.contains("fn scan_top_level_ahead") {
                summary
                    .bounded_scan_helpers
                    .push("scan_top_level_ahead".to_string());
            }
            if line.contains("fn recover_top_level_until") {
                summary
                    .bounded_scan_helpers
                    .push("recover_top_level_until".to_string());
            }
        }
    }

    if let Ok(source) = fs::read_to_string(&declarations_path) {
        for (index, line) in source.lines().enumerate() {
            let line_number = index + 1;
            if line.contains("fn has_top_level_comma_before_rparen")
                || line.contains("let mut depth")
            {
                summary
                    .declaration_scanner_violations
                    .push(SourcePatternSummary {
                        path: rel_path(root, &declarations_path),
                        line: line_number,
                        pattern: line.trim().to_string(),
                    });
            }
            if line.contains(
                "positional struct initializers are not supported; use named field initializers",
            ) {
                summary
                    .positional_diagnostic_sites
                    .push(SourcePatternSummary {
                        path: rel_path(root, &declarations_path),
                        line: line_number,
                        pattern: "POSITIONAL_INITIALIZER_DIAGNOSTIC".to_string(),
                    });
            }
        }
    }

    if let Ok(source) = fs::read_to_string(&parser_variables_path) {
        for line in source.lines() {
            let Some(name) = line
                .trim()
                .strip_prefix("fn ")
                .and_then(|rest| rest.split_once('(').map(|(name, _)| name))
            else {
                continue;
            };
            if name.contains("positional_initializer_recovery")
                || name.contains("initializer_recovery_property")
            {
                summary.property_tests.push(name.to_string());
            }
        }
    }

    summary
}

fn collect_diagram_facts(root: &Path, policy: &DiagramPolicy) -> Result<Vec<DiagramFact>> {
    let mut facts = Vec::new();
    for rel in &policy.selected_diagrams {
        let path = root.join(rel);
        let source =
            fs::read_to_string(&path).with_context(|| format!("read {}", path.display()))?;
        let mut components = Vec::new();
        let mut edges = Vec::new();
        for line in source.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('\'') || trimmed.starts_with("@") {
                continue;
            }
            if let Some(alias) = plantuml_alias(trimmed) {
                components.push(alias.to_string());
            }
            if let Some((from, to)) = plantuml_edge(trimmed) {
                edges.push(DiagramEdge { from, to });
            }
        }
        facts.push(DiagramFact {
            path: rel.clone(),
            components,
            edges,
        });
    }
    Ok(facts)
}

fn edge_tuple(from: &str, to: &str, kind: &str) -> (String, String, String) {
    (from.to_string(), to.to_string(), kind.to_string())
}

fn class_map(items: &[ClassifiedName]) -> BTreeMap<String, String> {
    items
        .iter()
        .map(|item| (item.name.clone(), item.class.clone()))
        .collect()
}

fn productish_class(class: &str) -> bool {
    matches!(class, "product" | "ui_product" | "conformance_benchmark")
}

fn command_to_module_name(command: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in command.chars().enumerate() {
        if ch.is_ascii_uppercase() && idx > 0 {
            out.push('_');
        }
        out.push(ch.to_ascii_lowercase());
    }
    out
}

fn runtime_module_for_path(runtime_src: &Path, bin_root: &Path, file: &Path) -> String {
    if let Ok(rel) = file.strip_prefix(bin_root) {
        return first_path_component_or_stem(rel);
    }
    if let Ok(rel) = file.strip_prefix(runtime_src) {
        let parts = rel
            .components()
            .map(|component| component.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<_>>();
        if parts.first().is_some_and(|part| part == "bin") {
            if let Some(second) = parts.get(1) {
                return second.trim_end_matches(".rs").to_string();
            }
        }
        return first_path_component_or_stem(rel);
    }
    file.file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or_default()
        .to_string()
}

fn first_path_component_or_stem(path: &Path) -> String {
    let mut components = path.components();
    let Some(first) = components.next() else {
        return String::new();
    };
    let value = first.as_os_str().to_string_lossy();
    if value.ends_with(".rs") {
        value.trim_end_matches(".rs").to_string()
    } else {
        value.to_string()
    }
}

fn source_line_modules(line: &str) -> Vec<String> {
    let mut modules = Vec::new();
    modules.extend(modules_after_marker(line, "crate::"));
    modules.extend(modules_after_marker(line, "trust_runtime::"));
    modules.sort();
    modules.dedup();
    modules
}

fn modules_after_marker(mut line: &str, marker: &str) -> Vec<String> {
    let mut modules = Vec::new();
    while let Some(idx) = line.find(marker) {
        let tail = &line[idx + marker.len()..];
        if let Some(inner) = tail.strip_prefix('{') {
            if let Some(end) = inner.find('}') {
                for item in inner[..end].split(',') {
                    if let Some(module) = first_identifier(item) {
                        if module != "self" && module != "super" {
                            modules.push(module.to_string());
                        }
                    }
                }
                line = &inner[end + 1..];
                continue;
            }
        }
        if let Some(module) = first_identifier(tail) {
            if module != "self" && module != "super" {
                modules.push(module.to_string());
            }
        }
        if tail.is_empty() {
            break;
        }
        line = &tail[1..];
    }
    modules
}

fn first_identifier(value: &str) -> Option<&str> {
    let trimmed = value.trim_start_matches(|ch: char| !is_ident_char(ch));
    leading_identifier(trimmed)
}

fn strip_line_comment(line: &str) -> &str {
    line.split_once("//").map_or(line, |(code, _)| code)
}

fn is_ident_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

fn is_runtime_bin_source(path: &str) -> bool {
    path.starts_with("crates/trust-runtime/src/bin/trust-runtime/")
}

fn is_test_source_file(path: &str) -> bool {
    path.contains("/tests/") || path.ends_with("/tests.rs") || path.ends_with("_test.rs")
}

fn crate_alias(crate_name: &str) -> String {
    format!("crate_{}", crate_name.replace('-', "_"))
}

fn diagram_alias_allowed(alias: &str, policy: &DiagramPolicy, map: &SoftwareMap) -> bool {
    if policy
        .allowed_aliases
        .iter()
        .any(|allowed| allowed == alias)
    {
        return true;
    }
    if policy
        .allowed_alias_prefixes
        .iter()
        .any(|prefix| alias.starts_with(prefix))
    {
        return true;
    }
    map.packages
        .iter()
        .any(|package| alias == crate_alias(&package.name))
        || map
            .runtime_top_level_modules
            .iter()
            .any(|module| alias == format!("rt_mod_{module}"))
        || map
            .runtime_bin_modules
            .iter()
            .any(|module| alias == format!("bin_{module}"))
}

fn plantuml_alias(line: &str) -> Option<&str> {
    let starters = [
        "component ",
        "package ",
        "actor ",
        "card ",
        "artifact ",
        "database ",
        "node ",
    ];
    if !starters.iter().any(|starter| line.starts_with(starter)) {
        return None;
    }
    let (_, alias_tail) = line.rsplit_once(" as ")?;
    leading_identifier(alias_tail.trim())
}

fn plantuml_edge(line: &str) -> Option<(String, String)> {
    let (left, right) = line.split_once("->").or_else(|| line.split_once("..>"))?;
    let left = left.split("-[").next().unwrap_or(left);
    let from = last_identifier(left)?;
    let to = first_identifier(right)?;
    Some((from.to_string(), to.to_string()))
}

fn last_identifier(value: &str) -> Option<&str> {
    let bytes = value.as_bytes();
    let mut end = bytes.len();
    while end > 0 && !is_ident_char(bytes[end - 1] as char) {
        end -= 1;
    }
    if end == 0 {
        return None;
    }
    let mut start = end;
    while start > 0 && is_ident_char(bytes[start - 1] as char) {
        start -= 1;
    }
    Some(&value[start..end])
}

fn command_version(command: &str, args: &[&str]) -> String {
    let output = Command::new(command).args(args).output();
    match output {
        Ok(output) if output.status.success() => {
            String::from_utf8_lossy(&output.stdout).trim().to_string()
        }
        Ok(output) => format!(
            "{command} unavailable: {}",
            String::from_utf8_lossy(&output.stderr).trim()
        ),
        Err(error) => format!("{command} unavailable: {error}"),
    }
}

fn cargo_metadata(root: &Path) -> Result<serde_json::Value> {
    let output = Command::new("cargo")
        .args(["metadata", "--all-features", "--format-version", "1"])
        .current_dir(root)
        .output()
        .context("run cargo metadata")?;
    if !output.status.success() {
        bail!(
            "cargo metadata failed:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    serde_json::from_slice(&output.stdout).context("parse cargo metadata JSON")
}

fn full_map_artifact_dir(root: &Path) -> Result<PathBuf> {
    let commit = Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .current_dir(root)
        .output()
        .context("resolve current git commit")?;
    let suffix = if commit.status.success() {
        String::from_utf8_lossy(&commit.stdout).trim().to_string()
    } else {
        "unknown".to_string()
    };
    Ok(root
        .join("target/gate-artifacts")
        .join(format!("full-software-map-{suffix}")))
}

fn collect_source_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    for rel in ["crates", "xtask", "scripts"] {
        let dir = root.join(rel);
        if dir.exists() {
            collect_source_files_inner(&dir, &mut files)?;
        }
    }
    Ok(files)
}

fn collect_runtime_top_level_modules(root: &Path) -> Result<Vec<String>> {
    let runtime_src = root.join("crates/trust-runtime/src");
    let mut modules = Vec::new();
    for entry in
        fs::read_dir(&runtime_src).with_context(|| format!("read {}", runtime_src.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name().to_string_lossy().to_string();
        if path.is_dir() {
            modules.push(name);
        } else if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) else {
                continue;
            };
            if !matches!(stem, "lib" | "main") {
                modules.push(stem.to_string());
            }
        }
    }
    Ok(modules)
}

fn collect_runtime_bin_modules(root: &Path) -> Result<Vec<String>> {
    let bin_dir = root.join("crates/trust-runtime/src/bin/trust-runtime");
    let mut modules = Vec::new();
    for entry in fs::read_dir(&bin_dir).with_context(|| format!("read {}", bin_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) == Some("rs") {
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                modules.push(stem.to_string());
            }
        }
    }
    Ok(modules)
}

fn collect_runtime_cli_actions(root: &Path) -> Result<Vec<CliActionSummary>> {
    let cli_dir = root.join("crates/trust-runtime/src/bin/trust-runtime/cli");
    let mut actions = Vec::new();
    for entry in fs::read_dir(&cli_dir).with_context(|| format!("read {}", cli_dir.display()))? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().and_then(|ext| ext.to_str()) != Some("rs") {
            continue;
        }
        let source = fs::read_to_string(&path)?;
        for enum_name in action_enum_names(&source) {
            actions.push(CliActionSummary {
                variants: parse_enum_variants(&source, &enum_name),
                name: enum_name,
            });
        }
    }
    Ok(actions)
}

fn collect_runtime_route_handlers(
    root: &Path,
    policy: &FullMapPolicy,
) -> Result<Vec<RuntimeRouteHandlerSummary>> {
    let mut handlers = Vec::new();
    let mut seen = BTreeSet::new();
    for route in &policy.runtime_command_module_routes {
        if !seen.insert(route.handler.clone()) {
            continue;
        }
        if let Some(handler) = find_runtime_route_handler(root, &route.handler)? {
            handlers.push(handler);
        }
    }
    Ok(handlers)
}

fn find_runtime_route_handler(
    root: &Path,
    handler: &str,
) -> Result<Option<RuntimeRouteHandlerSummary>> {
    let segments = handler
        .split("::")
        .filter(|segment| !segment.is_empty())
        .collect::<Vec<_>>();
    if segments.len() < 2 {
        return Ok(None);
    }
    let function_name = segments[segments.len() - 1];
    let module_segments = &segments[..segments.len() - 1];
    let (base_dir, module_segments) = if module_segments.first() == Some(&"trust_runtime") {
        (root.join("crates/trust-runtime/src"), &module_segments[1..])
    } else {
        (
            root.join("crates/trust-runtime/src/bin/trust-runtime"),
            module_segments,
        )
    };
    if module_segments.is_empty() {
        return Ok(None);
    }

    for file in route_handler_candidate_files(&base_dir, module_segments)? {
        let source = fs::read_to_string(&file).unwrap_or_default();
        for (idx, line) in source.lines().enumerate() {
            if line_defines_function(line, function_name) {
                return Ok(Some(RuntimeRouteHandlerSummary {
                    handler: handler.to_string(),
                    path: rel_path(root, &file),
                    line: idx + 1,
                }));
            }
        }
    }
    Ok(None)
}

fn route_handler_candidate_files(
    base_dir: &Path,
    module_segments: &[&str],
) -> Result<Vec<PathBuf>> {
    let mut module_path = base_dir.to_path_buf();
    for segment in module_segments {
        module_path.push(segment);
    }

    let mut files = Vec::new();
    let rs_file = module_path.with_extension("rs");
    if rs_file.exists() {
        files.push(rs_file);
    }
    if module_path.is_dir() {
        collect_source_files_inner(&module_path, &mut files)?;
    }
    if base_dir.ends_with("crates/trust-runtime/src") && module_segments.len() == 1 {
        let host_module = base_dir.join("host").join(module_segments[0]);
        let host_rs = host_module.with_extension("rs");
        if host_rs.exists() {
            files.push(host_rs);
        }
        let host_mod = host_module.join("mod.rs");
        if host_mod.exists() {
            files.push(host_mod);
        }
    }
    files.sort();
    files.dedup();
    Ok(files)
}

fn line_defines_function(line: &str, function_name: &str) -> bool {
    let line = strip_line_comment(line).trim_start();
    if line.is_empty() {
        return false;
    }
    let needle = format!("fn {function_name}");
    let Some(pos) = line.find(&needle) else {
        return false;
    };
    let after = &line[pos + needle.len()..];
    matches!(after.chars().next(), Some('(' | '<'))
}

fn action_enum_names(source: &str) -> Vec<String> {
    source
        .lines()
        .filter_map(|line| line.trim().strip_prefix("pub enum "))
        .filter_map(|tail| tail.split_whitespace().next())
        .map(|name| name.trim_end_matches('{').to_string())
        .filter(|name| name.ends_with("Action"))
        .collect()
}

fn parse_enum_variants(source: &str, enum_name: &str) -> Vec<String> {
    let mut variants = Vec::new();
    let mut in_enum = false;
    let mut brace_balance = 0isize;
    for line in source.lines() {
        let trimmed = line.trim();
        if !in_enum {
            if trimmed.starts_with(&format!("pub enum {enum_name}")) {
                in_enum = true;
                brace_balance += count_char(trimmed, '{') as isize;
                brace_balance -= count_char(trimmed, '}') as isize;
            }
            continue;
        }
        brace_balance += count_char(trimmed, '{') as isize;
        brace_balance -= count_char(trimmed, '}') as isize;
        if trimmed.starts_with("#[") || trimmed.starts_with("//") || trimmed.is_empty() {
            continue;
        }
        if let Some(name) = leading_identifier(trimmed) {
            if name.chars().next().is_some_and(char::is_uppercase) && name != enum_name {
                variants.push(name.to_string());
            }
        }
        if brace_balance <= 0 {
            break;
        }
    }
    variants
}

fn leading_identifier(line: &str) -> Option<&str> {
    let end = line
        .find(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .unwrap_or(line.len());
    if end == 0 {
        None
    } else {
        Some(&line[..end])
    }
}

fn count_char(value: &str, needle: char) -> usize {
    value.chars().filter(|ch| *ch == needle).count()
}

fn collect_source_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == "target" || name == "node_modules" || name == "__pycache__" {
            continue;
        }
        if path.is_dir() {
            collect_source_files_inner(&path, files)?;
        } else if matches!(
            path.extension().and_then(|ext| ext.to_str()),
            Some("rs" | "py" | "toml")
        ) {
            files.push(path);
        }
    }
    Ok(())
}

fn rel_path(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn audit_report_policy_failures(
        report_json: &str,
        allowlist: &[DependencyAllowlistEntry],
    ) -> Result<Vec<String>> {
        let report: serde_json::Value =
            serde_json::from_str(report_json).context("parse canned cargo audit report")?;
        let allowed = allowlist
            .iter()
            .map(|entry| (entry.id.clone(), entry.package.clone()))
            .collect::<BTreeSet<_>>();
        let mut failures = Vec::new();
        let vulnerabilities = report
            .pointer("/vulnerabilities/list")
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten();
        for vulnerability in vulnerabilities {
            let id = vulnerability
                .pointer("/advisory/id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing-id>");
            let package = vulnerability
                .pointer("/package/name")
                .or_else(|| vulnerability.pointer("/advisory/package"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("<missing-package>");
            if !allowed.contains(&(id.to_string(), package.to_string())) {
                failures.push(format!(
                    "cargo audit advisory {id} for {package} is not in audit_allowlist"
                ));
            }
        }
        Ok(failures)
    }

    fn machete_report_policy_failures(
        report: &str,
        allowlist: &[DependencyAllowlistEntry],
    ) -> Vec<String> {
        let mut current_package = None;
        let mut failures = Vec::new();
        for raw_line in report.lines() {
            let line = raw_line.trim();
            if let Some((package, manifest)) = line.split_once(" -- ") {
                if manifest.ends_with("Cargo.toml:") {
                    current_package = Some(package.to_string());
                }
                continue;
            }
            if !(raw_line.starts_with('\t') || raw_line.starts_with("    ")) || line.is_empty() {
                continue;
            }
            let package = current_package.as_deref().unwrap_or("<unknown-package>");
            let is_allowed = allowlist
                .iter()
                .any(|entry| entry.id == package && entry.package == line);
            if !is_allowed {
                failures.push(format!(
                    "cargo machete finding {package}:{line} is not in machete_allowlist"
                ));
            }
        }
        failures
    }

    fn deny_policy_metadata_failures_from_source(source: &str) -> Vec<String> {
        let manifest: toml::Value = match toml::from_str(source) {
            Ok(value) => value,
            Err(error) => return vec![format!("deny.toml is not valid TOML: {error}")],
        };
        let mut failures = Vec::new();
        let license_allow = manifest
            .get("licenses")
            .and_then(|section| section.get("allow"))
            .and_then(toml::Value::as_array);
        if license_allow.is_none_or(Vec::is_empty) {
            failures.push("[licenses].allow must be present and non-empty".to_string());
        }
        let Some(advisory_ignores) = manifest
            .get("advisories")
            .and_then(|section| section.get("ignore"))
            .and_then(toml::Value::as_array)
        else {
            failures.push("[advisories].ignore must be present".to_string());
            return failures;
        };
        for ignore in advisory_ignores {
            let id = ignore
                .get("id")
                .and_then(toml::Value::as_str)
                .unwrap_or("<missing-id>");
            let reason = ignore
                .get("reason")
                .and_then(toml::Value::as_str)
                .unwrap_or_default();
            for required in ["owner=", "rationale=", "review=", "removal="] {
                if !reason.contains(required) {
                    failures.push(format!(
                        "deny.toml advisory ignore {id} reason is missing {required}"
                    ));
                }
            }
        }
        failures
    }

    #[test]
    fn parses_enum_variants_without_attributes() {
        let source = r#"
            pub enum Command {
                #[command(alias = "serve")]
                Run {
                    project: Option<PathBuf>,
                },
                Bench {
                    action: BenchAction,
                },
                Completions {
                    shell: Shell,
                },
            }
        "#;

        assert_eq!(
            parse_enum_variants(source, "Command"),
            vec!["Run", "Bench", "Completions"]
        );
    }

    #[test]
    fn finds_action_enum_names() {
        let source = r#"
            pub enum BenchAction {
                Project,
            }
            pub enum NotACommand {
                Value,
            }
        "#;

        assert_eq!(action_enum_names(source), vec!["BenchAction"]);
    }

    #[test]
    fn known_bad_unknown_workspace_edge_fails_policy() {
        let mut map = base_map();
        map.workspace_edges.push(WorkspaceEdge {
            from: "trust-hir".to_string(),
            to: "trust-runtime".to_string(),
            kind: "normal".to_string(),
        });

        assert!(check_workspace_edge_policy(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_runtime_core_forbidden_dependency_fails() {
        let mut map = base_map();
        map.packages.push(PackageSummary {
            name: "trust-runtime-core".to_string(),
            manifest_path: "crates/trust-runtime-core/Cargo.toml".to_string(),
            targets: Vec::new(),
        });
        map.direct_dependencies.push(DependencyEdge {
            from: "trust-runtime-core".to_string(),
            to: "tokio".to_string(),
            kind: "normal".to_string(),
        });

        assert!(check_runtime_core_dependency_fence(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_runtime_core_forbidden_host_import_fails() {
        let mut map = base_map();
        map.import_edges.push(ImportEdge {
            from_file: "crates/trust-runtime-core/src/lib.rs".to_string(),
            from_module: "lib".to_string(),
            to_module: "web".to_string(),
            line: 3,
        });

        assert!(check_runtime_core_dependency_fence(&map, &base_policy()).is_fail());
    }

    #[test]
    fn repo_runtime_core_policy_covers_runtime_split_forbidden_sets() {
        let policy: FullMapPolicy =
            serde_json::from_str(include_str!("../config/full_map_policy.json"))
                .expect("parse repository full-map policy");
        let dependencies = policy
            .runtime_core_forbidden_dependencies
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for dependency in [
            "trust-runtime",
            "trust-ide",
            "trust-lsp",
            "trust-debug",
            "tokio",
            "zenoh",
            "rumqttc",
            "rustls",
            "tiny_http",
            "tungstenite",
            "mdns-sd",
            "notify",
            "opcua",
            "ethercrab",
            "ureq",
            "ratatui",
            "crossterm",
            "home",
        ] {
            assert!(
                dependencies.contains(dependency),
                "runtime-core forbidden dependency policy missing {dependency}"
            );
        }

        let imports = policy
            .runtime_core_forbidden_import_modules
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        for module in [
            "web",
            "hmi",
            "control",
            "runtime_cloud",
            "mesh",
            "discovery",
            "io",
            "opcua",
            "debug",
            "security",
            "setup",
            "simulation",
            "ui",
            "historian",
        ] {
            assert!(
                imports.contains(module),
                "runtime-core forbidden import policy missing {module}"
            );
        }
    }

    #[test]
    fn function_size_summaries_count_multiline_function_bodies() {
        let source = r#"
            pub(crate) fn tiny() {
                helper();
            }

            fn multiline_signature(
                value: i32,
            ) -> i32 {
                if value > 0 {
                    value
                } else {
                    0
                }
            }
        "#;

        let functions = function_summaries_from_source("crates/trust-runtime/src/demo.rs", source);

        assert_eq!(functions.len(), 2);
        assert!(functions
            .iter()
            .any(|function| function.name == "tiny" && function.line_count == 3));
        assert!(functions.iter().any(|function| {
            function.name == "multiline_signature" && function.line_count == 9
        }));
    }

    #[test]
    fn known_bad_runtime_core_large_function_fails_kiss_check() {
        let mut map = base_map();
        map.largest_functions.push(FunctionSummary {
            path: "crates/trust-runtime-core/src/value.rs".to_string(),
            line: 10,
            name: "oversized".to_string(),
            line_count: 250,
        });

        assert!(check_kiss_thresholds(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_large_module_without_owner_note_fails_kiss_check() {
        let mut map = base_map();
        map.crate_module_summaries.push(ModuleSummary {
            crate_name: "trust-runtime".to_string(),
            module_name: "giant".to_string(),
            path: "crates/trust-runtime/src/giant".to_string(),
            file_count: 7,
            line_count: 5000,
        });

        let check = check_kiss_thresholds(&map, &base_policy());

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("trust-runtime::giant")));
    }

    #[test]
    fn runtime_top_level_final_cap_waiver_allows_recorded_over_cap_baseline() {
        let mut map = base_map();
        map.runtime_top_level_modules.push("debug".to_string());
        let mut policy = base_policy();
        policy
            .kiss
            .runtime_top_level_module_decisions
            .push(RuntimeTopLevelModuleDecision {
                name: "debug".to_string(),
                subsystem: "debug_protocol".to_string(),
                owner: "runtime/debug".to_string(),
                rationale: "debug protocol surface".to_string(),
                review_date: "2026-05-02".to_string(),
                decision_note: "test decision".to_string(),
            });
        policy.kiss.runtime_top_level_module_cap_waiver = Some(RuntimeTopLevelModuleCapWaiver {
            target_cap: 2,
            owner: "runtime".to_string(),
            rationale: "test cap waiver".to_string(),
            review_date: "2026-05-02".to_string(),
            extraction_branch: "architecture/runtime-host-module-collapse".to_string(),
            removal_condition: "collapse debug under host modules".to_string(),
        });

        let check = check_kiss_thresholds(&map, &policy);

        assert!(!check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("final host cap waiver active")));
    }

    #[test]
    fn known_bad_missing_public_api_baseline_fails() {
        let mut map = base_map();
        map.tool_results.push(ToolResult {
            name: "cargo public-api".to_string(),
            status: ToolStatus::Pass,
            details: vec!["cargo-public-api test".to_string()],
        });
        let mut policy = base_policy();
        policy
            .kiss
            .public_api_snapshots
            .push(PublicApiSnapshotPolicy {
                package: "trust-runtime".to_string(),
                baseline: "docs/internal/architecture/public-api/missing.txt".to_string(),
                command:
                    "cargo public-api --manifest-path crates/trust-runtime/Cargo.toml --color never"
                        .to_string(),
                owner: "architecture automation".to_string(),
                rationale: "test baseline".to_string(),
                review_date: "2026-05-02".to_string(),
            });

        let check = check_public_api_snapshot_status(Path::new("/repo"), &map, &policy);

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("public API baseline")));
    }

    #[test]
    fn known_bad_unclassified_command_module_and_action_fail() {
        let mut map = base_map();
        map.runtime_cli_commands.push("NewCommand".to_string());
        map.runtime_bin_modules.push("new_module".to_string());
        map.runtime_cli_actions.push(CliActionSummary {
            name: "NewAction".to_string(),
            variants: vec!["Run".to_string()],
        });

        assert!(check_runtime_command_and_module_ownership(&map, &base_policy()).is_fail());
    }

    #[test]
    fn documented_command_route_replaces_mapping_question() {
        let mut map = base_map();
        map.runtime_cli_commands.push("Play".to_string());

        let mut policy = base_policy();
        policy.runtime_command_classes.push(ClassifiedName {
            name: "Play".to_string(),
            class: "product".to_string(),
            owner: "runtime".to_string(),
            rationale: "runtime command".to_string(),
        });
        policy
            .runtime_command_module_routes
            .push(CommandModuleRoute {
                command: "Play".to_string(),
                module: "run".to_string(),
                handler: "run::run_play".to_string(),
                route_kind: "compatibility_alias".to_string(),
                owner: "runtime CLI".to_string(),
                rationale: "Command::Play dispatches to run::run_play".to_string(),
                review_date: "2026-04-28".to_string(),
            });
        map.runtime_route_handlers.push(RuntimeRouteHandlerSummary {
            handler: "run::run_play".to_string(),
            path: "crates/trust-runtime/src/bin/trust-runtime/run/commands.rs".to_string(),
            line: 39,
        });

        let check = check_runtime_command_and_module_ownership(&map, &policy);

        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.details.iter().any(|detail| detail
            .contains("Command::Play has no same-name bin module 'play'; routes through run handler=run::run_play")));
        assert!(!check
            .details
            .iter()
            .any(|detail| detail.contains("may be routed")));
    }

    #[test]
    fn known_bad_stale_command_route_handler_fails() {
        let mut map = base_map();
        map.runtime_cli_commands.push("Play".to_string());

        let mut policy = base_policy();
        policy.runtime_command_classes.push(ClassifiedName {
            name: "Play".to_string(),
            class: "product".to_string(),
            owner: "runtime".to_string(),
            rationale: "runtime command".to_string(),
        });
        policy
            .runtime_command_module_routes
            .push(CommandModuleRoute {
                command: "Play".to_string(),
                module: "run".to_string(),
                handler: "run::missing_play".to_string(),
                route_kind: "compatibility_alias".to_string(),
                owner: "runtime CLI".to_string(),
                rationale: "stale handler".to_string(),
                review_date: "2026-04-28".to_string(),
            });

        assert!(check_runtime_command_and_module_ownership(&map, &policy).is_fail());
    }

    #[test]
    fn known_bad_product_bin_importing_workbench_module_fails() {
        let mut map = base_map();
        map.import_edges.push(ImportEdge {
            from_file: "crates/trust-runtime/src/bin/trust-runtime/run.rs".to_string(),
            from_module: "run".to_string(),
            to_module: "agent".to_string(),
            line: 7,
        });

        assert!(check_runtime_command_and_module_ownership(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_field_runtime_profile_including_workbench_fails() {
        let mut policy = base_policy();
        let profile = policy
            .runtime_artifact_profiles
            .iter_mut()
            .find(|profile| profile.class == "field_runtime")
            .expect("field runtime profile");
        profile.include_classes.push("workbench_dev".to_string());
        profile.exclude_classes.clear();

        assert!(check_runtime_command_and_module_ownership(&base_map(), &policy).is_fail());
    }

    #[test]
    fn known_bad_workbench_command_without_migration_policy_fails() {
        let mut map = base_map();
        map.runtime_cli_commands.push("Agent".to_string());

        let mut policy = base_policy();
        policy.runtime_command_classes.push(ClassifiedName {
            name: "Agent".to_string(),
            class: "workbench_dev".to_string(),
            owner: "dev tooling".to_string(),
            rationale: "agent command".to_string(),
        });

        assert!(check_runtime_command_and_module_ownership(&map, &policy).is_fail());
    }

    #[test]
    fn documented_workbench_command_migration_policy_passes() {
        let mut map = base_map();
        map.runtime_cli_commands.push("Agent".to_string());

        let mut policy = base_policy();
        policy.runtime_command_classes.push(ClassifiedName {
            name: "Agent".to_string(),
            class: "workbench_dev".to_string(),
            owner: "dev tooling".to_string(),
            rationale: "agent command".to_string(),
        });
        policy
            .runtime_workbench_command_migrations
            .push(RuntimeWorkbenchCommandMigration {
                command: "Agent".to_string(),
                current_binary: "trust-runtime".to_string(),
                destination_binary: "trust-dev".to_string(),
                compatibility_plan: "deprecated_forwarding_alias".to_string(),
                owner: "dev tooling".to_string(),
                rationale: "agent serve remains available through a compatibility alias"
                    .to_string(),
                review_date: "2026-05-01".to_string(),
            });

        let check = check_runtime_command_and_module_ownership(&map, &policy);

        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check.details.iter().any(|detail| detail
            .contains("workbench command 'Agent' migrates trust-runtime -> trust-dev")));
    }

    #[test]
    fn known_bad_host_surface_forbidden_import_fails_without_waiver() {
        let mut map = base_map();
        map.import_edges.push(ImportEdge {
            from_file: "crates/trust-runtime/src/control/hmi_handlers.rs".to_string(),
            from_module: "control".to_string(),
            to_module: "web".to_string(),
            line: 11,
        });

        assert!(check_host_surface_edges(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_runtime_cloud_importing_web_module_fails() {
        let mut map = base_map();
        map.import_edges.push(ImportEdge {
            from_file: "crates/trust-runtime/src/runtime_cloud/routing.rs".to_string(),
            from_module: "runtime_cloud".to_string(),
            to_module: "web".to_string(),
            line: 12,
        });

        assert!(check_host_surface_edges(&map, &base_policy()).is_fail());
    }

    #[test]
    fn host_surface_test_import_is_ignored() {
        let mut map = base_map();
        map.import_edges.push(ImportEdge {
            from_file: "crates/trust-runtime/src/control/tests/helpers.rs".to_string(),
            from_module: "control".to_string(),
            to_module: "web".to_string(),
            line: 11,
        });

        assert!(!check_host_surface_edges(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_host_surface_file_without_owner_category_fails() {
        let mut map = base_map();
        map.source_files.push(SourceFileSummary {
            path: "crates/trust-runtime/src/control.rs".to_string(),
            line_count: 120,
        });
        let mut policy = base_policy();
        policy.host_surface.owned_paths.clear();

        let check = check_host_surface_edges(&map, &policy);

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail
                .contains("crates/trust-runtime/src/control.rs' has no owner category")));
    }

    #[test]
    fn known_bad_web_route_direct_runtime_state_bypass_fails_when_ports_active() {
        let mut map = base_map();
        map.host_surface
            .direct_runtime_state_bypasses
            .push(SourcePatternSummary {
                path: "crates/trust-runtime/src/web/ui_routes.rs".to_string(),
                line: 51,
                pattern: "ControlState.project_root direct web access".to_string(),
            });
        let mut policy = base_policy();
        policy.host_surface.approved_ports_active = true;

        let check = check_host_surface_edges(&map, &policy);

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("bypasses approved host-surface port")));
    }

    #[test]
    fn known_bad_web_route_direct_control_dispatch_bypass_fails_when_ports_active() {
        let mut map = base_map();
        map.host_surface
            .direct_control_dispatch_bypasses
            .push(SourcePatternSummary {
                path: "crates/trust-runtime/src/web/auth_helpers.rs".to_string(),
                line: 82,
                pattern: "handle_request_value direct web dispatch".to_string(),
            });
        let mut policy = base_policy();
        policy.host_surface.approved_ports_active = true;

        let check = check_host_surface_edges(&map, &policy);

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("bypasses approved control-dispatch port")));
    }

    #[test]
    fn host_surface_ports_active_passes_without_direct_runtime_state_bypass() {
        let mut policy = base_policy();
        policy.host_surface.approved_ports_active = true;

        let check = check_host_surface_edges(&base_map(), &policy);

        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("direct web runtime-state bypass findings: 0")));
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("direct web control-dispatch bypass findings: 0")));
    }

    #[test]
    fn direct_control_state_field_bypass_detects_runtime_state_fields() {
        assert_eq!(
            direct_control_state_field_bypass("ctx.control_state.project_root.clone()"),
            Some("project_root")
        );
        assert_eq!(
            direct_control_state_field_bypass(
                "dispatch_control_request(payload, control_state, None)"
            ),
            None
        );
    }

    #[test]
    fn direct_control_dispatch_bypass_detects_handle_request_value() {
        assert!(direct_control_dispatch_bypass(
            "let response = handle_request_value(payload, state, client);"
        ));
        assert!(direct_control_dispatch_bypass(
            "use crate::control::handle_request_value;"
        ));
        assert!(!direct_control_dispatch_bypass(
            "let response = dispatch_control_request(payload, state, client, token);"
        ));
    }

    #[test]
    fn known_bad_failed_dependency_tool_status_fails() {
        let map = base_map();
        let mut policy = base_policy();
        policy.dependency_hygiene_tools[0].status = "failed".to_string();

        assert!(check_dependency_hygiene_status(&map, &policy).is_fail());
    }

    #[test]
    fn known_bad_dependency_allowlist_without_metadata_fails() {
        let mut policy = base_policy();
        policy.dependency_hygiene.audit_allowlist[0].owner.clear();

        assert!(check_policy_metadata(&policy).is_fail());
    }

    #[test]
    fn known_bad_unallowlisted_audit_report_fails() -> Result<()> {
        let report = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {"id": "RUSTSEC-2099-0001", "package": "demo-vulnerable"},
                        "package": {"name": "demo-vulnerable"}
                    }
                ]
            }
        }"#;

        assert!(!audit_report_policy_failures(
            report,
            &base_policy().dependency_hygiene.audit_allowlist,
        )?
        .is_empty());
        Ok(())
    }

    #[test]
    fn audit_report_allowlist_accepts_documented_advisory() -> Result<()> {
        let report = r#"{
            "vulnerabilities": {
                "list": [
                    {
                        "advisory": {"id": "RUSTSEC-0000-0000", "package": "example"},
                        "package": {"name": "example"}
                    }
                ]
            }
        }"#;

        assert!(audit_report_policy_failures(
            report,
            &base_policy().dependency_hygiene.audit_allowlist,
        )?
        .is_empty());
        Ok(())
    }

    #[test]
    fn known_bad_unallowlisted_machete_report_fails() {
        let report = "cargo-machete found the following unused dependencies in this directory:\n\
trust-runtime -- ./crates/trust-runtime/Cargo.toml:\n\
\tunused-demo\n";

        assert!(!machete_report_policy_failures(
            report,
            &base_policy().dependency_hygiene.machete_allowlist,
        )
        .is_empty());
    }

    #[test]
    fn machete_report_allowlist_accepts_documented_dependency() {
        let report = "cargo-machete found the following unused dependencies in this directory:\n\
trust-runtime -- ./crates/trust-runtime/Cargo.toml:\n\
\tunused-demo\n";
        let mut policy = base_policy();
        policy
            .dependency_hygiene
            .machete_allowlist
            .push(DependencyAllowlistEntry {
                id: "trust-runtime".to_string(),
                package: "unused-demo".to_string(),
                owner: "runtime".to_string(),
                rationale: "test fixture".to_string(),
                review_date: "2026-04-29".to_string(),
                removal_condition: "remove after fixture".to_string(),
            });

        assert!(machete_report_policy_failures(
            report,
            &policy.dependency_hygiene.machete_allowlist,
        )
        .is_empty());
    }

    #[test]
    fn current_deny_policy_has_required_sections_and_metadata() {
        let failures = deny_policy_metadata_failures_from_source(include_str!("../../deny.toml"));

        assert_eq!(failures, Vec::<String>::new());
    }

    #[test]
    fn known_bad_deny_policy_missing_metadata_fails() {
        let failures = deny_policy_metadata_failures_from_source(
            r#"
            [advisories]
            ignore = [
                { id = "RUSTSEC-2099-0001", reason = "owner=runtime" },
            ]

            [licenses]
            allow = ["MIT"]
            "#,
        );

        assert!(failures
            .iter()
            .any(|failure| failure.contains("rationale=")));
        assert!(failures.iter().any(|failure| failure.contains("review=")));
        assert!(failures.iter().any(|failure| failure.contains("removal=")));
    }

    #[test]
    fn known_bad_tiverse_workspace_status_mismatch_fails() {
        let mut map = base_map();
        map.dependency_hygiene.third_party_tiverse_mmap_status = "ambiguous".to_string();

        assert!(check_dependency_hygiene_status(&map, &base_policy()).is_fail());
    }

    #[test]
    fn workspace_exclude_manifest_marks_tiverse_standalone() {
        let excludes = workspace_excludes_from_manifest_source(
            r#"
            [workspace]
            members = ["crates/runtime"]
            exclude = ["third_party/tiverse-mmap"]
            "#,
        )
        .unwrap();
        let members = BTreeSet::new();

        assert_eq!(
            classify_workspace_path(
                Path::new("/repo"),
                &members,
                &excludes,
                "third_party/tiverse-mmap",
            ),
            "workspace_exclude"
        );
    }

    #[test]
    fn known_bad_large_runtime_file_without_owner_note_fails() {
        let mut map = base_map();
        map.source_files.push(SourceFileSummary {
            path: "crates/trust-runtime/src/new_large.rs".to_string(),
            line_count: 1001,
        });

        assert!(check_kiss_thresholds(&map, &base_policy()).is_fail());
    }

    #[test]
    fn top_level_module_summary_prefers_directory_for_split_module() -> Result<()> {
        let suffix = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!(
            "trust-full-map-module-summary-{}-{suffix}",
            std::process::id()
        ));
        let crate_dir = root.join("crates/demo");
        let src_dir = crate_dir.join("src");
        fs::create_dir_all(src_dir.join("web"))?;
        fs::write(src_dir.join("web.rs"), "mod routes;\npub fn serve() {}\n")?;
        fs::write(src_dir.join("web/routes.rs"), "pub fn route() {}\n")?;

        let mut summaries = Vec::new();
        collect_top_level_module_summaries(
            &root,
            "demo",
            &crate_dir.join("Cargo.toml"),
            &mut summaries,
        )?;
        fs::remove_dir_all(&root).ok();

        let web = summaries
            .iter()
            .find(|summary| summary.module_name == "web")
            .expect("web module summary");
        assert_eq!(web.path, "crates/demo/src/web");
        assert_eq!(web.file_count, 2);
        assert_eq!(web.line_count, 3);
        Ok(())
    }

    #[test]
    fn known_bad_large_runtime_test_file_without_owner_note_fails() {
        let mut map = base_map();
        map.source_files.push(SourceFileSummary {
            path: "crates/trust-runtime/tests/new_large.rs".to_string(),
            line_count: 1001,
        });

        let check = check_kiss_thresholds(&map, &base_policy());

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("crates/trust-runtime/tests/new_large.rs")));
    }

    #[test]
    fn known_bad_stale_large_file_allowlist_entry_fails() {
        let mut policy = base_policy();
        policy.kiss.large_file_allowlist.push(LargeFilePolicy {
            path: "crates/trust-runtime/src/missing_large.rs".to_string(),
            owner: "runtime".to_string(),
            rationale: "stale entry should not be accepted".to_string(),
            review_date: "2026-05-01".to_string(),
            split_plan: "RTLARGE test".to_string(),
        });

        let check = check_kiss_thresholds(&base_map(), &policy);

        assert!(check.is_fail());
        assert!(check.details.iter().any(|detail| detail.contains(
            "large-file allowlist entry 'crates/trust-runtime/src/missing_large.rs' does not match a current source file"
        )));
    }

    #[test]
    fn known_bad_runtime_top_level_module_without_decision_note_fails() {
        let mut map = base_map();
        map.runtime_top_level_modules
            .push("new_surface".to_string());

        let check = check_kiss_thresholds(&map, &base_policy());

        assert!(check.is_fail());
        assert!(check.details.iter().any(|detail| detail
            .contains("runtime top-level module 'new_surface' has no subsystem decision note")));
    }

    #[test]
    fn known_bad_runtime_top_level_module_decision_without_metadata_fails() {
        let mut policy = base_policy();
        policy.kiss.runtime_top_level_module_decisions[0]
            .decision_note
            .clear();

        assert!(check_policy_metadata(&policy).is_fail());
    }

    #[test]
    fn known_bad_unsafe_summary_without_owner_fails() {
        let mut map = base_map();
        map.unsafe_summary.owner.clear();

        assert!(check_unsafe_concurrency_summary(&map).is_fail());
    }

    #[test]
    fn known_bad_parser_recovery_ad_hoc_scanner_fails() {
        let mut map = base_map();
        map.parser_recovery
            .declaration_scanner_violations
            .push(SourcePatternSummary {
                path: "crates/trust-syntax/src/parser/grammar/declarations.rs".to_string(),
                line: 409,
                pattern: "let mut depth = 0usize;".to_string(),
            });

        assert!(check_parser_recovery_rules(&map).is_fail());
    }

    #[test]
    fn known_bad_parser_recovery_missing_property_test_fails() {
        let mut map = base_map();
        map.parser_recovery
            .property_tests
            .retain(|name| !name.contains("property_smoke"));

        assert!(check_parser_recovery_rules(&map).is_fail());
    }

    #[test]
    fn known_bad_hir_zero_doctor_finding_fails_full_map_check() {
        let check = hir_zero_silent_bug_doctor_check_from_output(
            "python3 scripts/hir_zero_silent_bug_doctor.py --fail",
            CommandCheckOutput {
                success: false,
                code: Some(1),
                stdout: "HIR zero-silent-bug doctor: 1 warn-only finding(s)\n\
                    HIRZSB-WARN-BROAD-LOOKUP crates/trust-hir/src/demo.rs:12: symbols.lookup_any(name)"
                    .to_string(),
                stderr: String::new(),
            },
        );

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("HIRZSB-WARN-BROAD-LOOKUP")));
    }

    #[test]
    fn hir_zero_doctor_no_findings_passes_full_map_check() {
        let check = hir_zero_silent_bug_doctor_check_from_output(
            "python3 scripts/hir_zero_silent_bug_doctor.py --fail",
            CommandCheckOutput {
                success: true,
                code: Some(0),
                stdout: "HIR zero-silent-bug doctor: no findings\n".to_string(),
                stderr: String::new(),
            },
        );

        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn runtime_safety_gate_findings_fail_full_map_check() {
        let check = runtime_safety_fail_closed_check_from_output(
            "./scripts/runtime_safety_fail_closed_ast_grep_gate.sh",
            CommandCheckOutput {
                success: true,
                code: Some(0),
                stdout: "runtime safety fail-closed gate: findings\n\
                    gate=runtime-safety-fail-closed\n\
                    phase=fail_class\n\
                    finding_count=3\n"
                    .to_string(),
                stderr: String::new(),
            },
        );

        assert_eq!(check.status, CheckStatus::Fail);
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("finding_count=3")));
    }

    #[test]
    fn runtime_safety_gate_no_findings_passes_full_map_check() {
        let check = runtime_safety_fail_closed_check_from_output(
            "./scripts/runtime_safety_fail_closed_ast_grep_gate.sh",
            CommandCheckOutput {
                success: true,
                code: Some(0),
                stdout: "runtime safety fail-closed gate: no findings\n\
                    gate=runtime-safety-fail-closed\n\
                    finding_count=0\n"
                    .to_string(),
                stderr: String::new(),
            },
        );

        assert_eq!(check.status, CheckStatus::Pass);
    }

    #[test]
    fn runtime_safety_gate_execution_error_fails_full_map_check() {
        let check = runtime_safety_fail_closed_check_from_output(
            "./scripts/runtime_safety_fail_closed_ast_grep_gate.sh",
            CommandCheckOutput {
                success: false,
                code: Some(2),
                stdout: String::new(),
                stderr: "allowlist exceeds max entries\n".to_string(),
            },
        );

        assert!(check.is_fail());
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("allowlist exceeds max entries")));
    }

    #[test]
    fn runtime_vm_mutation_clean_evidence_passes_full_map_check() {
        let check = runtime_vm_mutation_evidence_check_from_parts(
            vec![RuntimeVmMutationShardEvidence {
                shard: "register-ir-tier1-state".to_string(),
                total: 32,
                caught: 31,
                missed: 0,
                timeout: 0,
                unviable: 1,
                outcomes_path:
                    "target/gate-artifacts/runtime-vm-mutants/register-ir-tier1-state/mutants.out/outcomes.json"
                        .to_string(),
            }],
            Vec::new(),
            Vec::new(),
        );

        assert_eq!(check.status, CheckStatus::Pass);
        assert!(check
            .details
            .iter()
            .any(|detail| detail.contains("register-ir-tier1-state: 32 total")));
    }

    #[test]
    fn runtime_vm_mutation_missing_evidence_is_partial_full_map_check() {
        let check = runtime_vm_mutation_evidence_check_from_parts(
            Vec::new(),
            vec!["register-ir-tier1-state".to_string()],
            Vec::new(),
        );

        assert_eq!(check.status, CheckStatus::Partial);
    }

    #[test]
    fn known_bad_runtime_vm_mutation_survivor_fails_full_map_check() {
        let check = runtime_vm_mutation_evidence_check_from_parts(
            vec![RuntimeVmMutationShardEvidence {
                shard: "register-ir-tier1-state".to_string(),
                total: 32,
                caught: 30,
                missed: 1,
                timeout: 0,
                unviable: 1,
                outcomes_path:
                    "target/gate-artifacts/runtime-vm-mutants/register-ir-tier1-state/mutants.out/outcomes.json"
                        .to_string(),
            }],
            Vec::new(),
            Vec::new(),
        );

        assert!(check.is_fail());
        assert!(check.details.iter().any(|detail| detail.contains("missed")));
    }

    #[test]
    fn unsafe_summary_with_remaining_hotspots_is_a_finding() {
        let check = check_unsafe_concurrency_summary(&base_map());

        assert_eq!(check.status, CheckStatus::Finding);
    }

    #[test]
    fn known_bad_unregistered_unsafe_site_fails() {
        let mut map = base_map();
        map.unsafe_summary
            .unregistered_unsafe_sites
            .push(SourcePatternSummary {
                path: "crates/trust-runtime/src/new_unsafe.rs".to_string(),
                line: 12,
                pattern: "unsafe { unchecked() }".to_string(),
            });

        assert!(check_unsafe_concurrency_summary(&map).is_fail());
    }

    #[test]
    fn known_bad_unclassified_runtime_panic_like_site_fails() {
        let mut map = base_map();
        map.unsafe_summary
            .unclassified_panic_like_sites
            .push(SourcePatternSummary {
                path: "crates/trust-runtime/src/runtime/vm/new_hot_path.rs".to_string(),
                line: 34,
                pattern: "value.unwrap()".to_string(),
            });

        assert!(check_unsafe_concurrency_summary(&map).is_fail());
    }

    #[test]
    fn known_bad_unregistered_concurrency_boundary_fails() {
        let mut map = base_map();
        map.unsafe_summary
            .unregistered_concurrency_boundaries
            .push(SourcePatternSummary {
                path: "crates/trust-runtime/src/control/new_shared.rs".to_string(),
                line: 56,
                pattern: "Arc<Mutex<State>>".to_string(),
            });

        assert!(check_unsafe_concurrency_summary(&map).is_fail());
    }

    #[test]
    fn known_bad_missing_safety_tool_gate_fails() {
        let mut map = base_map();
        map.unsafe_summary.tool_gates.clear();

        assert!(check_unsafe_concurrency_summary(&map).is_fail());
    }

    #[test]
    fn known_bad_unsupported_diagram_alias_fails() {
        let mut map = base_map();
        map.diagram_facts.push(DiagramFact {
            path: "docs/diagrams/architecture/full-software-map-generated.puml".to_string(),
            components: vec!["made_up_component".to_string()],
            edges: Vec::new(),
        });

        assert!(check_diagram_claims(&map, &base_policy()).is_fail());
    }

    #[test]
    fn known_bad_unsupported_diagram_crate_edge_fails() {
        let mut map = base_map();
        map.packages.push(PackageSummary {
            name: "trust-hir".to_string(),
            manifest_path: "crates/trust-hir/Cargo.toml".to_string(),
            targets: Vec::new(),
        });
        map.diagram_facts.push(DiagramFact {
            path: "docs/diagrams/architecture/full-software-map-generated.puml".to_string(),
            components: vec![
                "crate_trust_runtime".to_string(),
                "crate_trust_hir".to_string(),
            ],
            edges: vec![DiagramEdge {
                from: "crate_trust_hir".to_string(),
                to: "crate_trust_runtime".to_string(),
            }],
        });

        assert!(check_diagram_claims(&map, &base_policy()).is_fail());
    }

    fn base_map() -> SoftwareMap {
        let mut map = SoftwareMap::new("/repo");
        map.packages.push(PackageSummary {
            name: "trust-runtime".to_string(),
            manifest_path: "crates/trust-runtime/Cargo.toml".to_string(),
            targets: Vec::new(),
        });
        map.workspace_edges.push(WorkspaceEdge {
            from: "trust-runtime".to_string(),
            to: "trust-hir".to_string(),
            kind: "normal".to_string(),
        });
        map.direct_dependencies.push(DependencyEdge {
            from: "trust-runtime".to_string(),
            to: "trust-hir".to_string(),
            kind: "normal".to_string(),
        });
        map.source_files.push(SourceFileSummary {
            path: "crates/trust-runtime/src/lib.rs".to_string(),
            line_count: 10,
        });
        map.runtime_top_level_modules = vec!["control".to_string(), "web".to_string()];
        map.runtime_cli_commands = vec!["Run".to_string()];
        map.runtime_bin_modules = vec!["run".to_string(), "agent".to_string()];
        map.runtime_cli_actions = vec![CliActionSummary {
            name: "AgentAction".to_string(),
            variants: vec!["Serve".to_string()],
        }];
        map.parser_recovery.bounded_scan_helpers = vec![
            "scan_top_level_ahead".to_string(),
            "recover_top_level_until".to_string(),
        ];
        map.parser_recovery
            .positional_diagnostic_sites
            .push(SourcePatternSummary {
                path: "crates/trust-syntax/src/parser/grammar/declarations.rs".to_string(),
                line: 12,
                pattern: "POSITIONAL_INITIALIZER_DIAGNOSTIC".to_string(),
            });
        map.parser_recovery.property_tests = vec![
            "test_positional_initializer_recovery_preserves_declaration_boundaries".to_string(),
            "test_initializer_recovery_property_smoke_for_generated_positional_shapes".to_string(),
        ];
        map.unsafe_summary = UnsafeSummary {
            unsafe_occurrences: 1,
            panic_like_occurrences: 1,
            concurrency_boundary_occurrences: 1,
            owner: "runtime".to_string(),
            status: "tracked".to_string(),
            production_unsafe_sites: vec![SourcePatternSummary {
                path: "crates/trust-runtime/src/unsafe_owner.rs".to_string(),
                line: 10,
                pattern: "unsafe { ffi_call() }".to_string(),
            }],
            production_panic_like_sites: vec![SourcePatternSummary {
                path: "crates/trust-runtime/src/web/ui_routes.rs".to_string(),
                line: 20,
                pattern: "Header::from_bytes(\"Content-Type\", \"application/json\").unwrap()"
                    .to_string(),
            }],
            concurrency_boundary_sites: vec![SourcePatternSummary {
                path: "crates/trust-runtime/src/scheduler/runner_loop.rs".to_string(),
                line: 30,
                pattern: "Arc<Mutex<ResourceState>>".to_string(),
            }],
            unregistered_unsafe_sites: Vec::new(),
            unclassified_panic_like_sites: Vec::new(),
            unregistered_concurrency_boundaries: Vec::new(),
            tool_gates: vec![SafetyToolGateSummary {
                name: "Miri focused shard".to_string(),
                status: "pass".to_string(),
                command: "scripts/unsafe_concurrency_miri_gate.sh".to_string(),
                evidence: "target/gate-artifacts".to_string(),
                blocker: String::new(),
            }],
        };
        map.dependency_hygiene = DependencyHygieneSummary {
            deny_policy_present: true,
            workspace_excludes: vec!["third_party/tiverse-mmap".to_string()],
            third_party_tiverse_mmap_status: "workspace_exclude".to_string(),
            audit_allowlist: Vec::new(),
            machete_allowlist: Vec::new(),
        };
        map
    }

    fn base_policy() -> FullMapPolicy {
        FullMapPolicy {
            policy_version: 1,
            review_date: "2026-04-28".to_string(),
            allowed_workspace_edges: vec![EdgePolicy {
                from: "trust-runtime".to_string(),
                to: "trust-hir".to_string(),
                kind: "normal".to_string(),
                status: "allowed".to_string(),
                owner: "runtime".to_string(),
                rationale: "runtime consumes HIR".to_string(),
            }],
            forbidden_workspace_edges: vec![EdgeKey {
                from: "trust-hir".to_string(),
                to: "trust-runtime".to_string(),
                kind: "normal".to_string(),
            }],
            runtime_core_forbidden_dependencies: vec!["tokio".to_string()],
            runtime_core_forbidden_import_modules: vec!["web".to_string()],
            runtime_command_classes: vec![ClassifiedName {
                name: "Run".to_string(),
                class: "product".to_string(),
                owner: "runtime".to_string(),
                rationale: "runtime command".to_string(),
            }],
            runtime_bin_module_classes: vec![
                ClassifiedName {
                    name: "run".to_string(),
                    class: "product".to_string(),
                    owner: "runtime".to_string(),
                    rationale: "runtime command".to_string(),
                },
                ClassifiedName {
                    name: "agent".to_string(),
                    class: "workbench_dev".to_string(),
                    owner: "dev tooling".to_string(),
                    rationale: "workbench command".to_string(),
                },
            ],
            runtime_action_classes: vec![ClassifiedName {
                name: "AgentAction".to_string(),
                class: "workbench_dev".to_string(),
                owner: "dev tooling".to_string(),
                rationale: "agent subcommands".to_string(),
            }],
            runtime_command_module_routes: Vec::new(),
            runtime_artifact_profiles: vec![RuntimeArtifactProfile {
                name: "field-runtime-minimal".to_string(),
                class: "field_runtime".to_string(),
                binaries: vec!["trust-runtime".to_string(), "trust-bundle-gen".to_string()],
                include_classes: vec![
                    "product".to_string(),
                    "ui_product".to_string(),
                    "support".to_string(),
                    "infrastructure".to_string(),
                ],
                exclude_classes: vec!["workbench_dev".to_string()],
                owner: "release engineering".to_string(),
                rationale: "field runtime artifacts must not grow workbench/dev command surface"
                    .to_string(),
                review_date: "2026-05-01".to_string(),
            }],
            runtime_workbench_command_migrations: Vec::new(),
            host_surface: HostSurfacePolicy {
                approved_ports_active: false,
                owned_paths: vec![
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/control.rs".to_string(),
                        category: "control_root".to_string(),
                        owner: "runtime/control".to_string(),
                        rationale: "control root".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/control/".to_string(),
                        category: "control_port".to_string(),
                        owner: "runtime/control".to_string(),
                        rationale: "control subtree".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/hmi.rs".to_string(),
                        category: "hmi_root".to_string(),
                        owner: "runtime/HMI".to_string(),
                        rationale: "HMI root".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/hmi/".to_string(),
                        category: "hmi_domain".to_string(),
                        owner: "runtime/HMI".to_string(),
                        rationale: "HMI subtree".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/web.rs".to_string(),
                        category: "web_root".to_string(),
                        owner: "runtime/web".to_string(),
                        rationale: "web root".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/web/".to_string(),
                        category: "web_adapter".to_string(),
                        owner: "runtime/web".to_string(),
                        rationale: "web subtree".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/ui.rs".to_string(),
                        category: "ui_root".to_string(),
                        owner: "runtime/UI".to_string(),
                        rationale: "UI root".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/ui/".to_string(),
                        category: "ui_presentation".to_string(),
                        owner: "runtime/UI".to_string(),
                        rationale: "UI subtree".to_string(),
                    },
                    HostSurfaceOwnedPath {
                        path_prefix: "crates/trust-runtime/src/runtime_cloud/".to_string(),
                        category: "runtime_cloud_domain".to_string(),
                        owner: "runtime-cloud".to_string(),
                        rationale: "runtime-cloud subtree".to_string(),
                    },
                ],
                forbidden_edges: vec![
                    ForbiddenModuleEdge {
                        from_module: "control".to_string(),
                        to_module: "web".to_string(),
                        owner: "runtime/web".to_string(),
                        rationale: "control must not depend on web".to_string(),
                    },
                    ForbiddenModuleEdge {
                        from_module: "hmi".to_string(),
                        to_module: "web".to_string(),
                        owner: "runtime/HMI".to_string(),
                        rationale: "HMI domain must not depend on web".to_string(),
                    },
                    ForbiddenModuleEdge {
                        from_module: "runtime_cloud".to_string(),
                        to_module: "web".to_string(),
                        owner: "runtime-cloud".to_string(),
                        rationale: "runtime-cloud domain must not depend on web".to_string(),
                    },
                ],
                temporary_allowlist: Vec::new(),
            },
            kiss: KissPolicy {
                new_file_line_limit: 1000,
                existing_file_note_limit: 1000,
                function_note_limit: 200,
                module_note_limit: 5000,
                module_split_plan_line_limit: 10000,
                split_plan_line_limit: 1500,
                max_runtime_top_level_modules_current: 5,
                max_runtime_top_level_modules_after_boards: 2,
                enforce_after_boards_cap: false,
                runtime_top_level_module_cap_waiver: None,
                runtime_top_level_module_decisions: vec![
                    RuntimeTopLevelModuleDecision {
                        name: "control".to_string(),
                        subsystem: "host_surface".to_string(),
                        owner: "runtime/control".to_string(),
                        rationale: "control command/query boundary".to_string(),
                        review_date: "2026-04-30".to_string(),
                        decision_note: "ARCHPROG-C-05 baseline".to_string(),
                    },
                    RuntimeTopLevelModuleDecision {
                        name: "web".to_string(),
                        subsystem: "host_surface".to_string(),
                        owner: "runtime/web".to_string(),
                        rationale: "HTTP/web adapter boundary".to_string(),
                        review_date: "2026-04-30".to_string(),
                        decision_note: "ARCHPROG-C-05 baseline".to_string(),
                    },
                ],
                large_file_allowlist: Vec::new(),
                module_size_allowlist: Vec::new(),
                function_size_allowlist: Vec::new(),
                public_api_snapshots: Vec::new(),
            },
            dependency_hygiene_tools: vec![PolicyToolStatus {
                name: "cargo audit".to_string(),
                status: "not_run".to_string(),
                owner: "release".to_string(),
                rationale: "board owns it".to_string(),
                review_date: "2026-04-29".to_string(),
                evidence: "target/gate-artifacts".to_string(),
            }],
            dependency_hygiene: DependencyHygienePolicy {
                third_party_tiverse_mmap: ThirdPartyWorkspacePolicy {
                    path: "third_party/tiverse-mmap".to_string(),
                    expected_status: "workspace_exclude".to_string(),
                    owner: "release engineering".to_string(),
                    rationale: "vendored patch crate is not a workspace member".to_string(),
                    review_date: "2026-04-29".to_string(),
                },
                audit_allowlist: vec![DependencyAllowlistEntry {
                    id: "RUSTSEC-0000-0000".to_string(),
                    package: "example".to_string(),
                    owner: "release".to_string(),
                    rationale: "test fixture".to_string(),
                    review_date: "2026-04-29".to_string(),
                    removal_condition: "remove after fixture".to_string(),
                }],
                machete_allowlist: Vec::new(),
            },
            unsafe_concurrency: UnsafeConcurrencyPolicy {
                owner: "runtime".to_string(),
                status: "tracked".to_string(),
                unsafe_site_register: vec![UnsafeSitePolicy {
                    path: "crates/trust-runtime/src/unsafe_owner.rs".to_string(),
                    line: 10,
                    owner: "runtime".to_string(),
                    invariant: "test unsafe invariant".to_string(),
                    test_evidence: "cargo test -p trust-runtime".to_string(),
                    review_date: "2026-05-02".to_string(),
                }],
                delegated_unsafe_path_register: vec![DelegatedUnsafePathPolicy {
                    path_prefix: "third_party/tiverse-mmap/".to_string(),
                    owner: "release engineering".to_string(),
                    invariant: "vendored unsafe is isolated behind typed mmap APIs".to_string(),
                    test_evidence: "cargo test -p tiverse-mmap".to_string(),
                    review_date: "2026-05-02".to_string(),
                }],
                panic_like_classifications: vec![PanicLikeClassificationPolicy {
                    path_prefix: "crates/trust-runtime/src/web/".to_string(),
                    pattern: "Header::from_bytes".to_string(),
                    classification: "static-header-construction".to_string(),
                    owner: "runtime/web".to_string(),
                    rationale: "test fixture".to_string(),
                    review_date: "2026-05-02".to_string(),
                }],
                concurrency_boundaries: vec![ConcurrencyBoundaryPolicy {
                    path_prefix: "crates/trust-runtime/src/scheduler/".to_string(),
                    primitive: "Mutex".to_string(),
                    owner: "runtime/scheduler".to_string(),
                    shared_state: "resource state".to_string(),
                    invariant: "state transitions are serialized".to_string(),
                    test_evidence: "cargo test -p trust-runtime scheduler".to_string(),
                    review_date: "2026-05-02".to_string(),
                }],
                tool_gates: vec![SafetyToolGatePolicy {
                    name: "Miri focused shard".to_string(),
                    status: "pass".to_string(),
                    command: "scripts/unsafe_concurrency_miri_gate.sh".to_string(),
                    evidence: "target/gate-artifacts".to_string(),
                    blocker: String::new(),
                    owner: "runtime".to_string(),
                    review_date: "2026-05-02".to_string(),
                }],
            },
            diagram_policy: DiagramPolicy {
                selected_diagrams: vec![
                    "docs/diagrams/architecture/full-software-map-generated.puml".to_string(),
                ],
                allowed_alias_prefixes: vec![
                    "crate_".to_string(),
                    "rt_".to_string(),
                    "bin_".to_string(),
                ],
                allowed_aliases: Vec::new(),
            },
        }
    }
}
