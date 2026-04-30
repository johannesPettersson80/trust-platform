use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitCode};

use anyhow::{anyhow, bail, Context, Result};
use serde::Serialize;
use serde_json::json;

mod full_map;
mod software_map;

use full_map::architecture_doctor_full_map;

fn main() -> ExitCode {
    match run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("xtask failed: {err:#}");
            ExitCode::FAILURE
        }
    }
}

fn run() -> Result<()> {
    let mut args = env::args().skip(1);
    let Some(command) = args.next() else {
        print_usage();
        return Ok(());
    };
    let root = workspace_root()?;
    match command.as_str() {
        "architecture-map" => architecture_map(&root),
        "architecture-doctor" => {
            let mode = args.next().unwrap_or_else(|| "--all".to_string());
            architecture_doctor(&root, &mode)
        }
        "-h" | "--help" | "help" => {
            print_usage();
            Ok(())
        }
        other => bail!("unknown xtask command '{other}'"),
    }
}

fn print_usage() {
    eprintln!("{}", usage());
}

fn usage() -> &'static str {
    "Usage:\n  cargo xtask architecture-map\n  cargo xtask architecture-doctor --all\n  cargo xtask architecture-doctor --changed\n  cargo xtask architecture-doctor --full-map"
}

fn workspace_root() -> Result<PathBuf> {
    let manifest_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    manifest_dir
        .parent()
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("xtask manifest has no parent"))
}

fn generated_dir(root: &Path) -> PathBuf {
    root.join("docs/internal/architecture/generated")
}

fn architecture_map(root: &Path) -> Result<()> {
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

    let metadata: serde_json::Value =
        serde_json::from_slice(&output.stdout).context("parse cargo metadata JSON")?;
    let workspace_members = metadata["workspace_members"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include workspace_members"))?
        .iter()
        .filter_map(serde_json::Value::as_str)
        .map(ToOwned::to_owned)
        .collect::<BTreeSet<_>>();

    let mut packages = Vec::new();
    for package in metadata["packages"]
        .as_array()
        .ok_or_else(|| anyhow!("cargo metadata did not include packages"))?
    {
        let id = package["id"].as_str().unwrap_or_default();
        if !workspace_members.contains(id) {
            continue;
        }
        let name = package["name"].as_str().unwrap_or_default();
        let manifest_path = package["manifest_path"].as_str().unwrap_or_default();
        let targets = package["targets"].as_array().cloned().unwrap_or_default();
        let dependencies = package["dependencies"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|dep| dep["name"].as_str())
            .filter(|dep| dep.starts_with("trust-"))
            .collect::<BTreeSet<_>>();
        packages.push(json!({
            "name": name,
            "manifest_path": manifest_path,
            "targets": targets.into_iter().map(|target| {
                json!({
                    "name": target["name"],
                    "kind": target["kind"],
                    "src_path": target["src_path"],
                })
            }).collect::<Vec<_>>(),
            "trust_dependencies": dependencies,
        }));
    }

    let source_files = collect_rs_files(root)?;
    let map = json!({
        "workspace_root": root,
        "package_count": packages.len(),
        "packages": packages,
        "rust_source_file_count": source_files.len(),
    });

    let out_dir = generated_dir(root);
    fs::create_dir_all(&out_dir)?;
    fs::write(
        out_dir.join("software-map.json"),
        serde_json::to_string_pretty(&map)?,
    )?;
    println!("wrote {}", out_dir.join("software-map.json").display());
    Ok(())
}

#[derive(Debug, Serialize)]
struct CheckResult {
    name: &'static str,
    status: CheckStatus,
    details: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
enum CheckStatus {
    Pass,
    Fail,
}

impl CheckResult {
    fn pass(name: &'static str, details: impl Into<Vec<String>>) -> Self {
        Self {
            name,
            status: CheckStatus::Pass,
            details: details.into(),
        }
    }

    fn fail(name: &'static str, details: impl Into<Vec<String>>) -> Self {
        Self {
            name,
            status: CheckStatus::Fail,
            details: details.into(),
        }
    }

    fn is_fail(&self) -> bool {
        matches!(self.status, CheckStatus::Fail)
    }
}

fn architecture_doctor(root: &Path, mode: &str) -> Result<()> {
    if mode == "--full-map" {
        return architecture_doctor_full_map(root);
    }
    if !matches!(mode, "--all" | "--changed") {
        bail!("architecture-doctor expects --all, --changed, or --full-map, got '{mode}'");
    }
    architecture_map(root)?;

    let mut checks = vec![
        check_parser_initializer_call_sites(root)?,
        check_positional_parser_contract(root)?,
        check_hir_runtime_boundary(root)?,
        check_runtime_initializer_no_cst(root)?,
        check_initializer_funnel(root)?,
        check_fb_runtime_symmetry(root)?,
        check_silent_drop_patterns(root)?,
        check_initializer_size_caps(root)?,
        check_snapshot_freshness(root)?,
        check_table_41_coverage(root)?,
    ];
    if mode == "--all" {
        checks.push(check_diagram_drift(root)?);
    }

    write_doctor_reports(root, mode, &checks)?;
    let failed = checks.iter().filter(|check| check.is_fail()).count();
    for check in &checks {
        let marker = if check.is_fail() { "FAIL" } else { "PASS" };
        println!("{marker}: {}", check.name);
        for detail in &check.details {
            println!("  - {detail}");
        }
    }

    if failed > 0 {
        bail!("architecture doctor found {failed} failing check(s)");
    }
    Ok(())
}

fn check_parser_initializer_call_sites(root: &Path) -> Result<CheckResult> {
    let allowed = [
        "crates/trust-syntax/src/parser/grammar/declarations.rs",
        "crates/trust-syntax/src/parser/grammar/expressions.rs",
        "crates/trust-syntax/src/parser/grammar/pou/pou_part_04.rs",
    ];
    let mut calls = Vec::new();
    for file in collect_rs_files(&root.join("crates/trust-syntax/src"))? {
        let rel = rel_path(root, &file);
        let source = fs::read_to_string(&file)?;
        for (idx, line) in source.lines().enumerate() {
            if line.contains("parse_var_initializer(") && !line.contains("fn parse_var_initializer")
            {
                calls.push(format!("{rel}:{}", idx + 1));
                if !allowed.contains(&rel.as_str()) {
                    return Ok(CheckResult::fail(
                        "parser initializer call sites",
                        vec![format!(
                            "unexpected parse_var_initializer call in {rel}:{}",
                            idx + 1
                        )],
                    ));
                }
            }
        }
    }
    if calls.len() != 6 {
        return Ok(CheckResult::fail(
            "parser initializer call sites",
            vec![format!(
                "expected 6 initializer-aware call sites, found {}: {}",
                calls.len(),
                calls.join(", ")
            )],
        ));
    }
    Ok(CheckResult::pass(
        "parser initializer call sites",
        vec![format!("allowed call sites: {}", calls.join(", "))],
    ))
}

fn check_positional_parser_contract(root: &Path) -> Result<CheckResult> {
    let path = root.join("crates/trust-syntax/src/parser/grammar/declarations.rs");
    let source = fs::read_to_string(path)?;
    let failures = positional_parser_contract_failures_from_source(&source);
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "positional initializer parser contract",
            vec!["dedicated positional branch and locked diagnostic found".to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "positional initializer parser contract",
            failures,
        ))
    }
}

fn positional_parser_contract_failures_from_source(source: &str) -> Vec<String> {
    let mut failures = Vec::new();
    if !source.contains("parse_positional_initializer_list") {
        failures.push("missing dedicated parse_positional_initializer_list branch".to_string());
    }
    if !source
        .contains("positional struct initializers are not supported; use named field initializers")
    {
        failures.push("missing locked positional diagnostic wording".to_string());
    }
    let has_legacy_comma_scan = source.contains("has_top_level_comma_before_rparen");
    let has_bounded_comma_scan = source.contains("scan_top_level_ahead(")
        && source.contains("BoundedTopLevelScan::Found(TokenKind::Comma)");
    if !has_legacy_comma_scan && !has_bounded_comma_scan {
        failures.push(
            "missing bounded top-level comma scan for non-numeric positional starts".to_string(),
        );
    }
    failures
}

fn check_hir_runtime_boundary(root: &Path) -> Result<CheckResult> {
    let hir_src = root.join("crates/trust-hir/src");
    let mut failures = Vec::new();
    for file in collect_rs_files(&hir_src)? {
        let source = fs::read_to_string(&file)?;
        for (idx, line) in source.lines().enumerate() {
            if line.contains("trust_runtime") || line.contains("trust-runtime") {
                failures.push(format!("{}:{}", rel_path(root, &file), idx + 1));
            }
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "HIR/runtime dependency boundary",
            vec!["no trust-runtime dependency found in trust-hir/src".to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "HIR/runtime dependency boundary",
            failures,
        ))
    }
}

fn check_runtime_initializer_no_cst(root: &Path) -> Result<CheckResult> {
    let mut failures = Vec::new();
    for path in [
        root.join("crates/trust-runtime/src/harness/initializer.rs"),
        root.join("crates/trust-runtime/src/harness/initializer/defaults.rs"),
    ] {
        let source = fs::read_to_string(&path)?;
        for (idx, line) in source.lines().enumerate() {
            if line.contains("SyntaxNode") || line.contains("trust_syntax") {
                failures.push(format!("{}:{}", rel_path(root, &path), idx + 1));
            }
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "runtime initializer CST boundary",
            vec!["initializer service does not import SyntaxNode/trust_syntax".to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "runtime initializer CST boundary",
            failures,
        ))
    }
}

fn check_initializer_funnel(root: &Path) -> Result<CheckResult> {
    let allowed = [
        "crates/trust-runtime/src/harness/coerce.rs",
        "crates/trust-runtime/src/harness/initializer.rs",
    ];
    let runtime_src = root.join("crates/trust-runtime/src");
    let mut failures = Vec::new();
    for file in collect_rs_files(&runtime_src)? {
        let rel = rel_path(root, &file);
        let source = fs::read_to_string(&file)?;
        for (idx, line) in source.lines().enumerate() {
            if line.contains("coerce_initializer_value_to_type(")
                && !allowed.contains(&rel.as_str())
            {
                failures.push(format!("{rel}:{}", idx + 1));
            }
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "runtime initializer service funnel",
            vec!["direct coerce_initializer_value_to_type calls are confined".to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "runtime initializer service funnel",
            failures,
        ))
    }
}

fn check_fb_runtime_symmetry(root: &Path) -> Result<CheckResult> {
    let required = [
        "crates/trust-runtime/src/instance.rs",
        "crates/trust-runtime/src/runtime/vm/local_init.rs",
        "crates/trust-runtime/src/eval/locals.rs",
    ];
    let mut failures = Vec::new();
    for rel in required {
        let source = fs::read_to_string(root.join(rel))?;
        if !source.contains("apply_fb_instance_initializer") {
            failures.push(format!(
                "{rel} does not call/apply FB instance initializers"
            ));
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "FB initializer runtime symmetry",
            vec!["normal runtime, VM local init, and eval locals reference FB initializer application".to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "FB initializer runtime symmetry",
            failures,
        ))
    }
}

fn check_silent_drop_patterns(root: &Path) -> Result<CheckResult> {
    let paths = [
        "crates/trust-hir/src/db/queries/collector/types.rs",
        "crates/trust-hir/src/db/symbol_import.rs",
        "crates/trust-runtime/src/harness/compiler/types.rs",
        "crates/trust-runtime/src/harness/compiler/vars.rs",
    ];
    let mut failures = Vec::new();
    for rel in paths {
        let source = fs::read_to_string(root.join(rel))?;
        for (idx, line) in source.lines().enumerate() {
            if line.contains("default_initializer: None") || contains_initializer_discard(line) {
                failures.push(format!("{rel}:{}: {}", idx + 1, line.trim()));
            }
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass(
            "silent initializer drop patterns",
            vec![
                "no forbidden default_initializer: None or _initializer discard in guarded paths"
                    .to_string(),
            ],
        ))
    } else {
        Ok(CheckResult::fail(
            "silent initializer drop patterns",
            failures,
        ))
    }
}

fn contains_initializer_discard(line: &str) -> bool {
    let trimmed = line.trim();
    trimmed.starts_with("let _initializer")
        || trimmed.contains("(_initializer")
        || trimmed.contains(", _initializer")
}

fn check_initializer_size_caps(root: &Path) -> Result<CheckResult> {
    let paths = [
        "crates/trust-runtime/src/harness/initializer.rs",
        "crates/trust-runtime/src/harness/initializer/defaults.rs",
    ];
    let mut failures = Vec::new();
    let mut details = Vec::new();
    for rel in paths {
        let path = root.join(rel);
        let source = fs::read_to_string(&path)?;
        let line_count = source.lines().count();
        details.push(format!("{rel}: {line_count} lines"));
        if line_count > 400 {
            failures.push(format!("{rel} has {line_count} lines; cap is 400"));
        }
        for (name, lines) in function_lengths(&source) {
            if lines > 60 {
                failures.push(format!("{rel}:{name} has {lines} lines; cap is 60"));
            }
        }
    }
    if failures.is_empty() {
        Ok(CheckResult::pass("initializer service size caps", details))
    } else {
        Ok(CheckResult::fail("initializer service size caps", failures))
    }
}

fn check_snapshot_freshness(root: &Path) -> Result<CheckResult> {
    let mut snap_new = Vec::new();
    for file in collect_files(root)? {
        if file.extension().and_then(|ext| ext.to_str()) == Some("new")
            && file
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with(".snap.new"))
        {
            snap_new.push(rel_path(root, &file));
        }
    }
    if snap_new.is_empty() {
        Ok(CheckResult::pass(
            "snapshot freshness",
            vec!["no .snap.new files found".to_string()],
        ))
    } else {
        Ok(CheckResult::fail("snapshot freshness", snap_new))
    }
}

fn check_table_41_coverage(root: &Path) -> Result<CheckResult> {
    let rel = "docs/specs/coverage/iec-table-test-map.toml";
    let source = fs::read_to_string(root.join(rel))?;
    if source.contains("Table 41")
        || source.contains("table = 41")
        || source.contains("id = \"41\"")
    {
        Ok(CheckResult::pass(
            "IEC Table 41 coverage map",
            vec![format!("{rel} includes Table 41 coverage")],
        ))
    } else {
        Ok(CheckResult::fail(
            "IEC Table 41 coverage map",
            vec![format!("{rel} does not mention Table 41")],
        ))
    }
}

fn check_diagram_drift(root: &Path) -> Result<CheckResult> {
    let output = Command::new("python")
        .args(["scripts/check_diagram_drift.py"])
        .current_dir(root)
        .output()
        .context("run diagram drift check")?;
    if output.status.success() {
        Ok(CheckResult::pass(
            "diagram drift",
            vec![String::from_utf8_lossy(&output.stdout).trim().to_string()],
        ))
    } else {
        Ok(CheckResult::fail(
            "diagram drift",
            vec![
                String::from_utf8_lossy(&output.stdout).trim().to_string(),
                String::from_utf8_lossy(&output.stderr).trim().to_string(),
            ],
        ))
    }
}

fn write_doctor_reports(root: &Path, mode: &str, checks: &[CheckResult]) -> Result<()> {
    let out_dir = generated_dir(root).join("reports");
    fs::create_dir_all(&out_dir)?;
    let failed = checks.iter().filter(|check| check.is_fail()).count();
    let json_report = json!({
        "mode": mode,
        "status": if failed == 0 { "pass" } else { "fail" },
        "failed": failed,
        "checks": checks,
    });
    fs::write(
        out_dir.join("architecture-doctor.json"),
        serde_json::to_string_pretty(&json_report)?,
    )?;

    let mut markdown = String::from("# Architecture Doctor Report\n\n");
    markdown.push_str(&format!(
        "- Mode: `{mode}`\n- Status: `{}`\n- Failed checks: `{failed}`\n\n",
        if failed == 0 { "pass" } else { "fail" }
    ));
    markdown.push_str("| Check | Status | Details |\n| --- | --- | --- |\n");
    for check in checks {
        let status = if check.is_fail() { "fail" } else { "pass" };
        let details = check.details.join("<br>");
        markdown.push_str(&format!("| {} | {} | {} |\n", check.name, status, details));
    }
    fs::write(out_dir.join("architecture-doctor.md"), markdown)?;
    Ok(())
}

fn function_lengths(source: &str) -> Vec<(String, usize)> {
    let lines = source.lines().collect::<Vec<_>>();
    let mut result = Vec::new();
    let mut idx = 0usize;
    while idx < lines.len() {
        let trimmed = lines[idx].trim_start();
        if !trimmed.contains("fn ") || trimmed.starts_with("//") {
            idx += 1;
            continue;
        }
        let Some(name) = function_name(trimmed) else {
            idx += 1;
            continue;
        };
        let start = idx;
        let mut brace_balance = 0isize;
        let mut seen_open = false;
        while idx < lines.len() {
            for ch in lines[idx].chars() {
                match ch {
                    '{' => {
                        brace_balance += 1;
                        seen_open = true;
                    }
                    '}' => brace_balance -= 1,
                    _ => {}
                }
            }
            idx += 1;
            if seen_open && brace_balance <= 0 {
                break;
            }
        }
        result.push((name, idx - start));
    }
    result
}

fn function_name(line: &str) -> Option<String> {
    let after_fn = line.split_once("fn ")?.1;
    let name = after_fn
        .split(|ch: char| !(ch.is_ascii_alphanumeric() || ch == '_'))
        .next()?;
    Some(name.to_string())
}

fn collect_rs_files(root: &Path) -> Result<Vec<PathBuf>> {
    Ok(collect_files(root)?
        .into_iter()
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("rs"))
        .filter(|path| {
            !path
                .components()
                .any(|component| component.as_os_str() == "target")
        })
        .collect())
}

fn collect_files(root: &Path) -> Result<Vec<PathBuf>> {
    let mut files = Vec::new();
    collect_files_inner(root, &mut files)?;
    Ok(files)
}

fn collect_files_inner(path: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    for entry in fs::read_dir(path).with_context(|| format!("read {}", path.display()))? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        if name == ".git" || name == "target" || name == "node_modules" {
            continue;
        }
        if path.is_dir() {
            collect_files_inner(&path, files)?;
        } else {
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

    #[test]
    fn usage_mentions_full_map_mode() {
        assert!(usage().contains("cargo xtask architecture-doctor --full-map"));
    }

    #[test]
    fn unknown_doctor_mode_error_mentions_full_map() {
        let err = architecture_doctor(Path::new("."), "--unknown").unwrap_err();
        let message = format!("{err:#}");

        assert!(message.contains("--full-map"));
    }

    #[test]
    fn positional_parser_contract_accepts_bounded_comma_scan() {
        let source = include_str!("../../crates/trust-syntax/src/parser/grammar/declarations.rs");

        assert_eq!(
            positional_parser_contract_failures_from_source(source),
            Vec::<String>::new()
        );
    }

    #[test]
    fn known_bad_positional_parser_contract_without_comma_scan_fails() {
        let source = r#"
            const POSITIONAL_INITIALIZER_DIAGNOSTIC: &str =
                "positional struct initializers are not supported; use named field initializers";

            fn parse_positional_initializer_list(&mut self) {}
        "#;

        assert!(positional_parser_contract_failures_from_source(source)
            .iter()
            .any(|failure| failure.contains("bounded top-level comma scan")));
    }
}
