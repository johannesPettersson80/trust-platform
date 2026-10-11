//! External AST facts checked against the same unsafe ownership scope as the doctor.
use super::{collect_safety_scan_files, FullMapPolicy, UnsafeConcurrencyPolicy};
use anyhow::{bail, ensure, Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const PATTERNS: [(&str, &str); 6] = [
    ("unsafe_block", "unsafe { $$$BODY }"),
    ("unsafe_impl", "unsafe impl $TRAIT for $TYPE { $$$BODY }"),
    (
        "unsafe_fn_no_return",
        "unsafe fn $NAME($$$ARGS) { $$$BODY }",
    ),
    (
        "unsafe_fn_return",
        "unsafe fn $NAME($$$ARGS) -> $RET { $$$BODY }",
    ),
    (
        "unsafe_fn_generic_return",
        "unsafe fn $NAME<$GENERIC>($$$ARGS) -> $RET { $$$BODY }",
    ),
    (
        "unsafe_fn_type_alias_system",
        "type $NAME = unsafe extern \"system\" fn($$$ARGS) -> $RET;",
    ),
];

#[derive(Debug, Deserialize)]
struct RawMatch {
    file: String,
    text: String,
    range: Range,
}
#[derive(Debug, Deserialize)]
struct Range {
    start: Position,
}
#[derive(Debug, Deserialize)]
struct Position {
    line: usize,
    column: usize,
}
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
struct Match {
    path: String,
    line: usize,
    column: usize,
    rule: String,
    text: String,
}

// Preserve the previous external scanner's exclusions, including its stricter
// treatment of other test-like filenames. Registered paths are added explicitly.
fn excluded_test_file(path: &str) -> bool {
    path.contains("/tests/") || path.contains("/test/") || path.ends_with("tests.rs")
}

/// Keep production scope complete, adding only exact reviewed test files.
fn inventory(root: &Path, policy: &UnsafeConcurrencyPolicy) -> Result<Vec<PathBuf>> {
    let mut files = collect_safety_scan_files(root)?
        .into_iter()
        .map(|path| {
            path.strip_prefix(root)
                .map(Path::to_path_buf)
                .context("source outside repository")
        })
        .collect::<Result<Vec<_>>>()?
        .into_iter()
        .filter(|path| !excluded_test_file(&path.to_string_lossy().replace('\\', "/")))
        .collect::<BTreeSet<_>>();
    for site in &policy.unsafe_site_register {
        let path = Path::new(&site.path);
        ensure!(
            path.components().all(|c| matches!(
                c,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )),
            "unsafe registration must be repository-relative: {}",
            site.path
        );
        if root.join(path).is_file() {
            files.insert(path.to_path_buf());
        }
    }
    Ok(files.into_iter().collect())
}

fn normalize(rule: &str, raw: RawMatch) -> Result<Match> {
    let offset = if rule == "unsafe_fn_type_alias_system" {
        raw.text
            .lines()
            .position(|line| line.contains("unsafe"))
            .unwrap_or(0)
    } else {
        0
    };
    Ok(Match {
        path: raw.file.replace('\\', "/"),
        line: raw
            .range
            .start
            .line
            .checked_add(1)
            .and_then(|line| line.checked_add(offset))
            .context("AST line overflow")?,
        column: raw
            .range
            .start
            .column
            .checked_add(1)
            .context("AST column overflow")?,
        rule: rule.to_owned(),
        text: raw.text,
    })
}

fn scanner() -> Result<String> {
    for tool in ["ast-grep", "sg"] {
        if let Ok(output) = Command::new(tool).arg("--version").output() {
            if output.status.success()
                && String::from_utf8_lossy(&output.stdout)
                    .to_ascii_lowercase()
                    .contains("ast-grep")
            {
                return Ok(std::env::var("AST_GREP_BIN").unwrap_or_else(|_| tool.to_owned()));
            }
        }
    }
    bail!("ast-grep is required; install ast-grep 0.42.1")
}

// ast-grep uses exit 1 for a normal empty match set. It is not an error exemption:
// only the complete, valid empty JSON response with no stderr establishes that case.
fn decode_scan_output(code: Option<i32>, stdout: &[u8], stderr: &[u8]) -> Result<Vec<Value>> {
    let facts: Vec<Value> = serde_json::from_slice(stdout).context("parse external AST facts")?;
    ensure!(
        code == Some(0) || (code == Some(1) && facts.is_empty() && stderr.is_empty()),
        "AST scanner failed (exit {code:?}): {}",
        String::from_utf8_lossy(stderr)
    );
    Ok(facts)
}

fn scan(root: &Path, files: &[PathBuf], tool: &str) -> Result<(Vec<Value>, Vec<Match>, String)> {
    let mut raw_facts = Vec::new();
    let mut matches = BTreeSet::new();
    for (rule, pattern) in PATTERNS {
        // Keep command lines bounded on every supported host.
        for batch in files.chunks(128) {
            let output = Command::new(tool)
                .current_dir(root)
                .args([
                    "run",
                    "--pattern",
                    pattern,
                    "--lang",
                    "rust",
                    "--json=compact",
                ])
                .args(batch)
                .output()
                .with_context(|| format!("run external AST pattern {rule}"))?;
            let facts = decode_scan_output(output.status.code(), &output.stdout, &output.stderr)
                .with_context(|| format!("external AST pattern {rule}"))?;
            for fact in &facts {
                matches.insert(normalize(
                    rule,
                    serde_json::from_value(fact.clone()).context("malformed AST location")?,
                )?);
            }
            raw_facts.extend(facts);
        }
    }
    let mut unsupported = String::new();
    for batch in files.chunks(128) {
        let output = Command::new("rg")
            .current_dir(root)
            .args([
                "-n",
                r#"\bunsafe\s+(trait\b|extern\s*("[^"]+")?\s*\{)"#,
                "--",
            ])
            .args(batch)
            .output()
            .context("scan unsupported unsafe forms")?;
        ensure!(
            output.status.success() || output.status.code() == Some(1),
            "unsupported-form scan failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        unsupported.push_str(
            std::str::from_utf8(&output.stdout).context("non-UTF-8 unsupported-form evidence")?,
        );
    }
    Ok((raw_facts, matches.into_iter().collect(), unsupported))
}

fn is_registered(site: &Match, policy: &UnsafeConcurrencyPolicy) -> bool {
    policy
        .unsafe_site_register
        .iter()
        .any(|registered| registered.path == site.path && registered.line == site.line)
}
fn is_delegated(site: &Match, policy: &UnsafeConcurrencyPolicy) -> bool {
    policy
        .delegated_unsafe_path_register
        .iter()
        .any(|entry| site.path.starts_with(&entry.path_prefix))
}
fn rejected(matches: &[Match], policy: &UnsafeConcurrencyPolicy) -> (Vec<Match>, Vec<Value>) {
    let unknown = matches
        .iter()
        .filter(|site| !is_registered(site, policy) && !is_delegated(site, policy))
        .cloned()
        .collect();
    let actual: BTreeSet<_> = matches
        .iter()
        .map(|site| (site.path.as_str(), site.line))
        .collect();
    let missing = policy.unsafe_site_register.iter()
        .filter(|site| !actual.contains(&(site.path.as_str(), site.line)))
        .map(|site| json!({"path":site.path,"line":site.line,"owner":site.owner,"invariant":site.invariant,"test_evidence":site.test_evidence,"review_date":site.review_date}))
        .collect();
    (unknown, missing)
}
fn write_json(path: &Path, value: &impl Serialize) -> Result<()> {
    fs::write(path, serde_json::to_string_pretty(value)? + "\n")
        .with_context(|| format!("write {}", path.display()))
}

pub fn external_safety_gate(root: &Path) -> Result<()> {
    let policy = FullMapPolicy::load(root)?;
    let policy = &policy.unsafe_concurrency;
    let head = Command::new("git")
        .current_dir(root)
        .args(["rev-parse", "--short", "HEAD"])
        .output()?;
    ensure!(head.status.success(), "cannot identify scanner source");
    let commit = String::from_utf8(head.stdout)?.trim().to_owned();
    let output = root.join(format!(
        "target/gate-artifacts/architecture-external-safety-{commit}"
    ));
    fs::create_dir_all(&output)?;
    let files = inventory(root, policy)?;
    let tool = scanner()?;
    let version = Command::new(&tool)
        .arg("--version")
        .output()
        .context("identify executed AST scanner")?;
    ensure!(
        version.status.success(),
        "cannot identify executed AST scanner"
    );
    let version = String::from_utf8(version.stdout)?.trim().to_owned();
    let (raw, matches, unsupported) = scan(root, &files, &tool)?;
    let (unknown, missing) = rejected(&matches, policy);
    write_json(&output.join("ast-grep-unsafe-raw.json"), &raw)?;
    write_json(&output.join("ast-grep-unsafe-normalized.json"), &matches)?;
    write_json(&output.join("ast-grep-unregistered-unsafe.json"), &unknown)?;
    write_json(
        &output.join("ast-grep-missing-registered-unsafe.json"),
        &missing,
    )?;
    fs::write(
        output.join("ast-grep-unsupported-unsafe-forms.txt"),
        &unsupported,
    )?;
    write_json(&output.join("scan-inventory.json"), &files)?;
    let paths = format!("scanner={version}\nraw_json={}\nnormalized_json={}\nunregistered_json={}\nmissing_registered_json={}\nunsupported_forms={}\n", output.join("ast-grep-unsafe-raw.json").display(), output.join("ast-grep-unsafe-normalized.json").display(), output.join("ast-grep-unregistered-unsafe.json").display(), output.join("ast-grep-missing-registered-unsafe.json").display(), output.join("ast-grep-unsupported-unsafe-forms.txt").display());
    let summary = format!("# Architecture external safety ast-grep gate\ncommit={commit}\n{paths}policy=xtask/config/full_map_policy.json\nsource_files={}\nmatched_unsafe_constructs={}\nregistered_first_party_matches={}\ndelegated_path_matches={}\nunregistered_matches={}\nmissing_registered_sites={}\nunsupported_unsafe_forms={}\n\nregistered_policy=unsafe_site_register exact file:line entries\ndelegated_policy=delegated_unsafe_path_register path prefixes\ndecision=pass only when every AST unsafe construct is registered or delegated and every exact registered site still exists\n", files.len(), matches.len(), matches.iter().filter(|s| is_registered(s, policy)).count(), matches.iter().filter(|s| !is_registered(s, policy) && is_delegated(s, policy)).count(), unknown.len(), missing.len(), unsupported.lines().count());
    fs::write(output.join("ast-grep-unsafe-summary.txt"), &summary)?;
    println!("{summary}");
    for site in &unknown {
        eprintln!(
            "unregistered {}:{}:{} [{}] {}",
            site.path,
            site.line,
            site.column,
            site.rule,
            site.text.replace('\n', " ")
        );
    }
    for site in &missing {
        eprintln!(
            "missing registered {}:{} owner={}",
            site["path"], site["line"], site["owner"]
        );
    }
    if !unsupported.is_empty() {
        eprintln!("unsupported unsafe forms:\n{unsupported}");
    }
    ensure!(
        unknown.is_empty() && missing.is_empty() && unsupported.is_empty(),
        "external unsafe ownership gate failed; retained reports: {}",
        output.display()
    );
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::super::{DelegatedUnsafePathPolicy, UnsafeSitePolicy};
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    struct Fixture(PathBuf);
    impl Fixture {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!(
                "unsafe-scope-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            fs::create_dir_all(&root).unwrap();
            Self(root)
        }
        fn source(&self, name: &str) {
            let path = self.0.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, "unsafe fn probe() {}\n").unwrap();
        }
    }
    impl Drop for Fixture {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
    fn policy(paths: &[&str]) -> UnsafeConcurrencyPolicy {
        UnsafeConcurrencyPolicy {
            owner: "test".into(),
            status: "reviewed".into(),
            unsafe_site_register: paths
                .iter()
                .map(|path| UnsafeSitePolicy {
                    path: (*path).into(),
                    line: 1,
                    owner: "test".into(),
                    invariant: "fixture".into(),
                    test_evidence: "native".into(),
                    review_date: "2026-10-10".into(),
                })
                .collect(),
            delegated_unsafe_path_register: vec![],
            panic_like_classifications: vec![],
            concurrency_boundaries: vec![],
            tool_gates: vec![],
        }
    }
    fn site(path: &str, line: usize) -> Match {
        Match {
            path: path.into(),
            line,
            column: 1,
            rule: "unsafe_fn_no_return".into(),
            text: "unsafe fn probe() {}".into(),
        }
    }
    #[test]
    fn scanner_includes_firmware_and_exact_registered_tests_without_scanning_other_tests() {
        let fixture = Fixture::new();
        let prod = "crates/demo/src/main.rs";
        let firmware = "firmware/demo/src/main.rs";
        let registered = "crates/demo/tests/reviewed.rs";
        let unrelated = "crates/demo/tests/unrelated.rs";
        for path in [prod, firmware, registered, unrelated] {
            fixture.source(path);
        }
        let policy = policy(&[prod, firmware, registered]);
        assert_eq!(
            inventory(&fixture.0, &policy).unwrap(),
            [prod, registered, firmware].map(PathBuf::from)
        );
        let facts = [prod, registered, firmware].map(|path| site(path, 1));
        assert_eq!(rejected(&facts, &policy), (vec![], vec![]));
    }
    #[test]
    fn unregistered_firmware_and_missing_or_shifted_registered_sites_fail_closed() {
        let policy = policy(&["firmware/demo/src/main.rs", "crates/demo/tests/reviewed.rs"]);
        let (unknown, missing) = rejected(
            &[
                site("firmware/demo/src/new.rs", 1),
                site("firmware/demo/src/main.rs", 2),
            ],
            &policy,
        );
        assert_eq!(unknown.len(), 2);
        assert_eq!(missing.len(), 2);
        assert_eq!(missing[0]["path"], "firmware/demo/src/main.rs");
        assert_eq!(missing[1]["path"], "crates/demo/tests/reviewed.rs");
    }
    #[test]
    fn reviewed_delegated_paths_are_accepted_without_accepting_adjacent_paths() {
        let mut policy = policy(&[]);
        policy
            .delegated_unsafe_path_register
            .push(DelegatedUnsafePathPolicy {
                path_prefix: "third_party/reviewed/".into(),
                owner: "test".into(),
                invariant: "fixture".into(),
                test_evidence: "native".into(),
                review_date: "2026-10-10".into(),
            });
        let (unknown, missing) = rejected(
            &[
                site("third_party/reviewed/lib.rs", 1),
                site("third_party/reviewed_other/lib.rs", 1),
            ],
            &policy,
        );
        assert!(missing.is_empty());
        assert_eq!(unknown, [site("third_party/reviewed_other/lib.rs", 1)]);
    }
    #[test]
    fn empty_match_exit_is_accepted_only_with_complete_empty_json_and_no_error() {
        assert!(decode_scan_output(Some(1), b"[]\n", b"")
            .unwrap()
            .is_empty());
        assert!(decode_scan_output(Some(0), b"[]", b"").unwrap().is_empty());
        assert_eq!(decode_scan_output(Some(0), b"[{}]", b"").unwrap().len(), 1);
        assert!(decode_scan_output(Some(0), b"malformed", b"").is_err());
        assert!(decode_scan_output(Some(1), b"[{}]", b"").is_err());
        assert!(decode_scan_output(Some(1), b"[]", b"scanner error").is_err());
        assert!(decode_scan_output(Some(1), b"", b"").is_err());
        assert!(decode_scan_output(Some(1), b"null", b"").is_err());
        assert!(decode_scan_output(Some(2), b"[]", b"").is_err());
        assert!(decode_scan_output(None, b"[]", b"").is_err());
    }

    #[test]
    fn malformed_external_locations_are_rejected_and_type_alias_offsets_are_preserved() {
        assert!(
            serde_json::from_value::<RawMatch>(json!({"file":"x.rs","text":"unsafe {}"})).is_err()
        );
        let raw: RawMatch = serde_json::from_value(json!({"file":"x.rs","text":"type F =\n unsafe extern \"system\" fn();","range":{"start":{"line":4,"column":2}}})).unwrap();
        let normalized = normalize("unsafe_fn_type_alias_system", raw).unwrap();
        assert_eq!((normalized.line, normalized.column), (6, 3));
        let overflow = RawMatch {
            file: "x.rs".into(),
            text: "unsafe {}".into(),
            range: Range {
                start: Position {
                    line: usize::MAX,
                    column: 0,
                },
            },
        };
        assert!(normalize("unsafe_block", overflow).is_err());
    }
}
